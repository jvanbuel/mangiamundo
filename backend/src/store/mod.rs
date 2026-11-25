//! SQLite persistence and the area cache.

mod areas;
mod places;
mod progress;
mod subscriptions;
mod visits;

pub use areas::{areas_containing, areas_of_place, find_area, save_area, Area};
pub use places::{area_places, place, place_summary, report_closed, Hours, HoursStatus, PlaceView};
pub use progress::{area_rows, lifetime, reward_quests, tasted, unreward_quest};
pub use subscriptions::{
    forget_countries, mark_push_sent, push_subscriptions, remove_push_subscription,
    remove_user_push_subscription, save_push_subscription, take_new_countries, unmark_push_sent,
};
pub use visits::{add_visit, remove_visit, visited_countries, visits, visits_between, Visit};

use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::catalog;
use crate::game::Bonus;
use crate::geo::{self, Bbox, Shape};
use crate::providers::{FoundPlace, Lookup};
use areas::upsert_place;

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs().cast_signed())
}

pub async fn connect(url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    // An in-memory database exists per connection, so it must never get a second one.
    let max = if url.contains(":memory:") { 1 } else { 5 };
    let pool = SqlitePoolOptions::new()
        .max_connections(max)
        .connect_with(options)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

/// Brings the database up to date: the migrations, then what they leave to the server.
pub(crate) async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!().run(pool).await?;
    let hashed = crate::account::hash_plain_tokens(pool).await?;
    if hashed > 0 {
        tracing::info!(hashed, "session tokens are now kept as hashes");
    }
    Ok(())
}

pub async fn setting(db: &SqlitePool, key: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(db)
        .await
}

pub async fn set_setting(db: &SqlitePool, key: &str, value: &str) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(key)
        .bind(value)
        .execute(db)
        .await?;
    Ok(())
}

/// A restaurant a user adds because OpenStreetMap lacks it.
pub struct NewPlace {
    pub name: String,
    pub lat: f64,
    pub lng: f64,
    pub address: Option<String>,
    pub url: Option<String>,
    pub country: &'static str,
    pub cuisine: &'static str,
    /// The unknown place this suggests the food of, from [`unknown_places`].
    pub suggested_for: Option<String>,
}

/// A place a user just added, and the reward it earns once OpenStreetMap has it.
pub struct AddedPlace {
    pub id: String,
    pub bonus: Bonus,
    /// The area the bonus was judged in, if one was cached.
    pub area_name: Option<String>,
}

/// Saves a place added by `user` and puts it in the cached areas around it. Returns it, or
/// `None` when the user already added `limit` places in the day before `at`. Checking and
/// inserting is one statement, so concurrent requests can't both take the last slot.
///
/// Its bonus is decided now, against the places everyone sees in its city: pending
/// additions don't count, so a mapper adding the same cuisine while it waits doesn't take
/// the bonus away. Only a city counts, never a postal code (which would make a "new
/// country" easier to find); with no cached city around it there is no bonus.
pub async fn add_place(
    db: &SqlitePool,
    user: &str,
    place: &NewPlace,
    at: i64,
    limit: i64,
) -> sqlx::Result<Option<AddedPlace>> {
    let id = format!("user-{}", uuid::Uuid::new_v4().simple());
    let mut tx = db.begin().await?;
    let added = sqlx::query(
        "INSERT INTO places (id, name, lat, lng, address, url, cuisine, updated_at, added_by, added_at, suggested_for)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?8, ?11
         WHERE (SELECT COUNT(*) FROM places WHERE added_by = ?9 AND added_at > ?8 - 86400) < ?10",
    )
    .bind(&id)
    .bind(&place.name)
    .bind(place.lat)
    .bind(place.lng)
    .bind(&place.address)
    .bind(&place.url)
    .bind(place.cuisine)
    .bind(at)
    .bind(user)
    .bind(limit)
    .bind(&place.suggested_for)
    .execute(&mut *tx)
    .await?;
    if added.rows_affected() == 0 {
        return Ok(None);
    }
    sqlx::query("INSERT INTO place_countries (place_id, country) VALUES (?, ?)")
        .bind(&id)
        .bind(place.country)
        .execute(&mut *tx)
        .await?;
    let mut home: Option<(String, String)> = None;
    for (area_id, name, kind) in put_in_areas(&mut tx, &id, place.lat, place.lng).await? {
        if kind == "city" {
            home.get_or_insert((area_id, name));
        }
    }
    let bonus = match &home {
        Some((area_id, _)) => {
            let present: Vec<String> = sqlx::query_scalar(
                "SELECT DISTINCT pc.country FROM area_places ap
                 JOIN places p ON p.id = ap.place_id
                 JOIN place_countries pc ON pc.place_id = p.id
                 WHERE ap.area_id = ? AND p.added_by IS NULL
                   AND (SELECT COUNT(*) FROM closed_reports WHERE place_id = p.id) < 2",
            )
            .bind(area_id)
            .fetch_all(&mut *tx)
            .await?;
            bonus_for(place.country, &present)
        }
        None => Bonus::None,
    };
    let (area_id, area_name) = home.unzip();
    sqlx::query(
        "INSERT INTO additions (place_id, user_id, name, country, area_id, area_name, claimed_bonus, bonus, submitted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8)",
    )
    .bind(&id)
    .bind(user)
    .bind(&place.name)
    .bind(place.country)
    .bind(&area_id)
    .bind(&area_name)
    .bind(bonus.as_str())
    .bind(at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(AddedPlace {
        id,
        bonus,
        area_name,
    }))
}

/// The bonus for adding a place of `country` to an area serving `present`.
fn bonus_for(country: &str, present: &[String]) -> Bonus {
    let continent = |code: &str| catalog::country(code).map(|c| c.continent.as_str());
    if present.iter().any(|c| c == country) {
        Bonus::None
    } else if present.iter().any(|c| continent(c) == continent(country)) {
        Bonus::Country
    } else {
        Bonus::Continent
    }
}

pub async fn set_osm_note(db: &SqlitePool, place_id: &str, note_id: i64) -> sqlx::Result<()> {
    sqlx::query("UPDATE places SET osm_note_id = ? WHERE id = ?")
        .bind(note_id)
        .bind(place_id)
        .execute(db)
        .await?;
    Ok(())
}

/// Replaces the imported OpenStreetMap places in `bbox`, and marks the cached areas centred
/// in it as stale so their next request rebuilds them from the new copy. Pending places
/// that the import now has are swapped for it; returns how many.
pub async fn replace_imported(
    db: &SqlitePool,
    bbox: Bbox,
    places: &[FoundPlace],
) -> sqlx::Result<usize> {
    let mut tx = db.begin().await?;
    // ponytail: by centre, so an area straddling the box edge is refetched live once per import.
    sqlx::query(
        "UPDATE areas SET fetched_at = 0 WHERE lat BETWEEN ? AND ? AND lng BETWEEN ? AND ?",
    )
    .bind(bbox.south)
    .bind(bbox.north)
    .bind(bbox.west)
    .bind(bbox.east)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM osm_places")
        .execute(&mut *tx)
        .await?;
    for place in places {
        keep_imported(&mut tx, place).await?;
    }
    let matched = match_pending(&mut tx, now()).await?;
    tx.commit().await?;
    Ok(matched)
}

/// Saves a place in the local copy of OpenStreetMap, so the next area rebuild uses it.
async fn keep_imported(tx: &mut sqlx::SqliteConnection, place: &FoundPlace) -> sqlx::Result<()> {
    let Some(cuisine) = &place.cuisine else {
        return Ok(());
    };
    sqlx::query(
        "INSERT OR REPLACE INTO osm_places (id, name, lat, lng, address, url, cuisine, opening_hours)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&place.id)
    .bind(&place.name)
    .bind(place.lat)
    .bind(place.lng)
    .bind(&place.address)
    .bind(&place.url)
    .bind(cuisine)
    .bind(&place.opening_hours)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Puts a place in every cached area whose outline holds it, and returns those areas as
/// (id, name, kind), by id.
async fn put_in_areas(
    tx: &mut sqlx::SqliteConnection,
    place_id: &str,
    lat: f64,
    lng: f64,
) -> sqlx::Result<Vec<(String, String, String)>> {
    // Areas are at most 1.5° across, so any area holding the place is centred within that.
    let near = sqlx::query("SELECT id, name, kind, shape FROM areas WHERE lat BETWEEN ? AND ? AND lng BETWEEN ? AND ? ORDER BY id")
        .bind(lat - 1.5)
        .bind(lat + 1.5)
        .bind(lng - 1.5)
        .bind(lng + 1.5)
        .fetch_all(&mut *tx)
        .await?;
    let mut inside = Vec::new();
    for r in near {
        if serde_json::from_str::<Shape>(r.get("shape")).is_ok_and(|s| s.contains(lat, lng)) {
            let area_id: String = r.get("id");
            sqlx::query("INSERT OR IGNORE INTO area_places (area_id, place_id) VALUES (?, ?)")
                .bind(&area_id)
                .bind(place_id)
                .execute(&mut *tx)
                .await?;
            inside.push((area_id, r.get("name"), r.get("kind")));
        }
    }
    Ok(inside)
}

/// What reading a place again from OpenStreetMap changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Refreshed {
    /// Its name, position, address, website, cuisine or hours changed, or it is listed again.
    Updated,
    /// OpenStreetMap has the same as we do. A very recent edit may not have reached it yet.
    Unchanged,
    /// No longer on OpenStreetMap as a restaurant serving a national cuisine: hidden now.
    Gone,
}

/// Applies a place read again from OpenStreetMap, to the place and to the local copy, so
/// the next import or area rebuild doesn't bring back the old version. A gone place leaves
/// every area but keeps its row, so visits to it are never lost.
pub async fn refresh_place(
    db: &SqlitePool,
    id: &str,
    lookup: &Lookup,
    at: i64,
) -> sqlx::Result<Refreshed> {
    let mut tx = db.begin().await?;
    let listed: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM area_places WHERE place_id = ?)")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    let outcome = match lookup {
        Lookup::Gone => {
            sqlx::query("DELETE FROM area_places WHERE place_id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM osm_places WHERE id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            Refreshed::Gone
        }
        Lookup::Found(place) => {
            let before = snapshot(&mut tx, id).await?;
            upsert_place(&mut tx, place, at).await?;
            keep_imported(&mut tx, place).await?;
            // It may have moved: only the areas holding its new position keep it.
            sqlx::query("DELETE FROM area_places WHERE place_id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            put_in_areas(&mut tx, id, place.lat, place.lng).await?;
            let after = snapshot(&mut tx, id).await?;
            if listed && before == after {
                Refreshed::Unchanged
            } else {
                Refreshed::Updated
            }
        }
    };
    tx.commit().await?;
    Ok(outcome)
}

/// What users see of a place, to tell whether a refresh changed it.
async fn snapshot(tx: &mut sqlx::SqliteConnection, id: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar(
        "SELECT json_array(name, round(lat, 6), round(lng, 6), address, url, cuisine, opening_hours,
                (SELECT group_concat(country) FROM (SELECT country FROM place_countries WHERE place_id = ?1 ORDER BY country)))
         FROM places WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
}

/// Saves places found around a point, puts them in the cached areas they lie in and
/// matches pending additions against them straight away. Returns the ids of those that
/// weren't listed in any area before, and how many pending places were matched.
pub async fn add_nearby(
    db: &SqlitePool,
    places: &[FoundPlace],
    at: i64,
) -> sqlx::Result<(Vec<String>, usize)> {
    let mut tx = db.begin().await?;
    let mut new = Vec::new();
    for place in places {
        let listed: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM area_places WHERE place_id = ?)")
                .bind(&place.id)
                .fetch_one(&mut *tx)
                .await?;
        upsert_place(&mut tx, place, at).await?;
        keep_imported(&mut tx, place).await?;
        let areas = put_in_areas(&mut tx, &place.id, place.lat, place.lng).await?;
        if !listed && !areas.is_empty() {
            new.push(place.id.clone());
        }
    }
    let matched = match_pending(&mut tx, at).await?;
    tx.commit().await?;
    Ok((new, matched))
}

/// How close an imported place must be to a pending one to be the same restaurant.
const MATCH_RADIUS_M: f64 = 50.0;

/// Swaps each pending place that now has an imported twin (same country, similar name,
/// within [`MATCH_RADIUS_M`]) for that twin: its visits and areas move over and the
/// pending row goes. `at` is when they are confirmed. Returns how many were swapped.
async fn match_pending(tx: &mut sqlx::SqliteConnection, at: i64) -> sqlx::Result<usize> {
    let pending = sqlx::query(
        "SELECT p.id, p.name, p.lat, p.lng, p.suggested_for, pc.country FROM places p
         JOIN place_countries pc ON pc.place_id = p.id
         WHERE p.added_by IS NOT NULL ORDER BY p.added_at, p.id",
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut matched = 0;
    for p in pending {
        let (id, name, lat, lng, country): (String, String, f64, f64, String) = (
            p.get("id"),
            p.get("name"),
            p.get("lat"),
            p.get("lng"),
            p.get("country"),
        );
        let suggested_for: Option<String> = p.get("suggested_for");
        // 0.001° of latitude is 111 m; a degree of longitude shrinks with cos(latitude).
        let dlng = 0.001 / lat.to_radians().cos().max(0.01);
        let near = sqlx::query(
            "SELECT * FROM osm_places WHERE lat BETWEEN ? AND ? AND lng BETWEEN ? AND ?",
        )
        .bind(lat - 0.001)
        .bind(lat + 0.001)
        .bind(lng - dlng)
        .bind(lng + dlng)
        .fetch_all(&mut *tx)
        .await?;
        let twin = near
            .iter()
            .filter_map(found)
            .filter(|o| o.countries.contains(&country.as_str()) && similar_names(&o.name, &name))
            .map(|o| (geo::distance_m(lat, lng, o.lat, o.lng), o))
            .filter(|(d, _)| *d <= MATCH_RADIUS_M)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        // A suggestion is about one OSM place: once that has a cuisine of its own, OSM wins.
        let overruled = match (&twin, suggested_for) {
            (Some(_), _) | (None, None) => None,
            (None, Some(osm_id)) => sqlx::query("SELECT * FROM osm_places WHERE id = ?")
                .bind(osm_id)
                .fetch_optional(&mut *tx)
                .await?
                .as_ref()
                .and_then(found),
        };
        let twin = match (twin, overruled) {
            (Some((_, twin)), _) => {
                confirm_addition(&mut *tx, &id, &twin.id, at).await?;
                twin
            }
            (None, Some(osm)) => {
                sqlx::query("DELETE FROM additions WHERE place_id = ?")
                    .bind(&id)
                    .execute(&mut *tx)
                    .await?;
                osm
            }
            (None, None) => continue,
        };
        upsert_place(&mut *tx, &twin, at).await?;
        // A clash means the user already has that row for the twin; the pending copy goes with the place.
        for sql in [
            "UPDATE OR IGNORE visits SET place_id = ?1 WHERE place_id = ?2",
            "UPDATE OR IGNORE closed_reports SET place_id = ?1 WHERE place_id = ?2",
            "INSERT OR IGNORE INTO area_places (area_id, place_id) SELECT area_id, ?1 FROM area_places WHERE place_id = ?2",
            "DELETE FROM places WHERE id = ?2",
        ] {
            sqlx::query(sql).bind(&twin.id).bind(&id).execute(&mut *tx).await?;
        }
        matched += 1;
    }
    Ok(matched)
}

/// Marks an addition as confirmed. Only the first addition of a place to be confirmed earns
/// anything; later ones are duplicates. Then the bonuses of its city are settled again.
async fn confirm_addition(
    tx: &mut sqlx::SqliteConnection,
    id: &str,
    osm_id: &str,
    at: i64,
) -> sqlx::Result<()> {
    let Some(area_id) =
        sqlx::query_scalar::<_, Option<String>>("SELECT area_id FROM additions WHERE place_id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
    else {
        return Ok(());
    };
    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM additions WHERE osm_place_id = ? AND rewarded)",
    )
    .bind(osm_id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE additions SET confirmed_at = ?, osm_place_id = ?, rewarded = ?,
             bonus = CASE WHEN ? THEN 'none' ELSE claimed_bonus END
         WHERE place_id = ?",
    )
    .bind(at)
    .bind(osm_id)
    .bind(!duplicate)
    .bind(duplicate)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if let Some(area_id) = area_id {
        settle_bonuses(&mut *tx, &area_id, id).await?;
    }
    Ok(())
}

/// Hands out a city's country and continent bonuses in submission order: an addition keeps
/// the bonus it claimed unless an earlier one, pending or rewarded, holds it. Duplicates hold
/// nothing, so a bonus whose claimant turned out to be a duplicate goes to the next in line.
/// `just_confirmed` takes the bonus it is due; others already confirmed only ever gain, so XP
/// that was granted and announced never goes down.
async fn settle_bonuses(
    tx: &mut sqlx::SqliteConnection,
    area_id: &str,
    just_confirmed: &str,
) -> sqlx::Result<()> {
    let rows: Vec<(String, String, String, String, bool)> = sqlx::query_as(
        "SELECT place_id, country, claimed_bonus, bonus, confirmed_at IS NOT NULL FROM additions
         WHERE area_id = ? AND (confirmed_at IS NULL OR rewarded)
         ORDER BY submitted_at, place_id",
    )
    .bind(area_id)
    .fetch_all(&mut *tx)
    .await?;
    let continent = |code: &str| catalog::country(code).map(|c| c.continent.as_str());
    let (mut countries, mut continents) = (Vec::new(), Vec::new());
    for (id, country, claimed, current, confirmed) in rows {
        let due = match Bonus::parse(&claimed) {
            Bonus::None => Bonus::None,
            _ if countries.contains(&country) => Bonus::None,
            Bonus::Continent if continents.contains(&continent(&country)) => Bonus::Country,
            claimed => claimed,
        };
        let current = Bonus::parse(&current);
        let settled = if !confirmed || id == just_confirmed {
            due
        } else {
            due.max(current)
        };
        if settled != Bonus::None {
            countries.push(country.clone());
        }
        if settled == Bonus::Continent {
            continents.push(continent(&country));
        }
        if confirmed && settled != current {
            sqlx::query("UPDATE additions SET bonus = ? WHERE place_id = ?")
                .bind(settled.as_str())
                .bind(&id)
                .execute(&mut *tx)
                .await?;
        }
    }
    Ok(())
}

/// Where a restaurant the user added stands.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AdditionStatus {
    /// Waiting for OpenStreetMap to have it.
    Pending,
    /// On OpenStreetMap; the reward was granted.
    Confirmed,
    /// On OpenStreetMap, but someone else added it first, so it earned nothing.
    Duplicate,
}

/// A restaurant the user added and its reward.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Addition {
    /// The addition's own id, which never changes. Two additions of the same place share
    /// a `place_id` once confirmed, but not this.
    pub id: String,
    /// The pending place's id, or once confirmed its OpenStreetMap twin's.
    pub place_id: String,
    pub name: String,
    pub country: String,
    /// The city its bonus was judged in. `None` when no city around it was cached.
    #[schema(required = true)]
    pub area_name: Option<String>,
    pub bonus: Bonus,
    /// What it earns once confirmed, or earned: 0 for a duplicate.
    pub xp: u32,
    pub status: AdditionStatus,
    pub submitted_at: i64,
    #[schema(required = true)]
    pub confirmed_at: Option<i64>,
    /// Whether the user was already told it is confirmed. Always `false` while pending.
    pub seen: bool,
}

/// The user's additions, newest first.
pub async fn additions(db: &SqlitePool, user: &str) -> sqlx::Result<Vec<Addition>> {
    let rows = sqlx::query(
        "SELECT place_id AS addition_id, COALESCE(osm_place_id, place_id) AS id, name, country, area_name, bonus, submitted_at,
                confirmed_at, rewarded, seen
         FROM additions WHERE user_id = ? ORDER BY submitted_at DESC, place_id",
    )
    .bind(user)
    .fetch_all(db)
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            let bonus = Bonus::parse(r.get("bonus"));
            let confirmed_at: Option<i64> = r.get("confirmed_at");
            let status = match (confirmed_at, r.get::<bool, _>("rewarded")) {
                (None, _) => AdditionStatus::Pending,
                (Some(_), true) => AdditionStatus::Confirmed,
                (Some(_), false) => AdditionStatus::Duplicate,
            };
            Addition {
                id: r.get("addition_id"),
                place_id: r.get("id"),
                name: r.get("name"),
                country: r.get("country"),
                area_name: r.get("area_name"),
                bonus,
                xp: if status == AdditionStatus::Duplicate {
                    0
                } else {
                    bonus.xp()
                },
                status,
                submitted_at: r.get("submitted_at"),
                confirmed_at,
                seen: r.get("seen"),
            }
        })
        .collect())
}

/// Marks some of the user's confirmed additions as told, by the `id` that [`additions`]
/// lists. Others stay unseen, such as one confirmed after the user was told.
pub async fn mark_additions_seen(db: &SqlitePool, user: &str, ids: &[String]) -> sqlx::Result<()> {
    for id in ids {
        sqlx::query("UPDATE additions SET seen = 1 WHERE user_id = ? AND confirmed_at IS NOT NULL AND place_id = ?")
        .bind(user)
        .bind(id)
        .execute(db)
        .await?;
    }
    Ok(())
}

/// The same name once case, punctuation and extra words like "Restaurant" are ignored:
/// one name's words all appear in the other.
// ponytail: no accent folding or typo tolerance ("Café" ≠ "Cafe"), add if matches are missed.
fn similar_names(a: &str, b: &str) -> bool {
    let words = |s: &str| -> Vec<String> {
        s.split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_lowercase)
            .collect()
    };
    let (a, b) = (words(a), words(b));
    let within = |x: &[String], y: &[String]| !x.is_empty() && x.iter().all(|w| y.contains(w));
    within(&a, &b) || within(&b, &a)
}

/// An `osm_places` row, or `None` when its cuisine maps to no country.
fn found(r: &sqlx::sqlite::SqliteRow) -> Option<FoundPlace> {
    let cuisine: String = r.get("cuisine");
    let countries = catalog::countries_for_cuisine_tag(&cuisine);
    (!countries.is_empty()).then(|| FoundPlace {
        id: r.get("id"),
        name: r.get("name"),
        lat: r.get("lat"),
        lng: r.get("lng"),
        address: r.get("address"),
        url: r.get("url"),
        cuisine: Some(cuisine),
        opening_hours: r.get("opening_hours"),
        countries,
    })
}

/// A named restaurant OpenStreetMap gives no cuisine, so it isn't on the map yet.
#[derive(Debug, Clone, PartialEq, Serialize, utoipa::ToSchema)]
pub struct UnknownPlace {
    /// An OpenStreetMap place id, like the ones of other places.
    pub id: String,
    pub name: String,
    pub lat: f64,
    pub lng: f64,
    #[schema(required = true)]
    pub address: Option<String>,
    pub distance_m: u32,
}

/// Replaces the restaurants with unknown food with a fresh import of them.
pub async fn replace_unknown(db: &SqlitePool, places: &[UnknownPlace]) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM unknown_places")
        .execute(&mut *tx)
        .await?;
    for p in places {
        sqlx::query(
            "INSERT OR REPLACE INTO unknown_places (id, name, lat, lng, address) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&p.id)
        .bind(&p.name)
        .bind(p.lat)
        .bind(p.lng)
        .bind(&p.address)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

fn unknown_place(r: &sqlx::sqlite::SqliteRow) -> UnknownPlace {
    UnknownPlace {
        id: r.get("id"),
        name: r.get("name"),
        lat: r.get("lat"),
        lng: r.get("lng"),
        address: r.get("address"),
        distance_m: 0,
    }
}

/// Restaurants with unknown food inside a box that `user` hasn't answered for yet, and that
/// fewer than two users said serve nothing specific. `distance_m` is left at 0.
pub async fn unknown_in(
    db: &SqlitePool,
    b: Bbox,
    user: Option<&str>,
) -> sqlx::Result<Vec<UnknownPlace>> {
    let rows = sqlx::query(
        "SELECT * FROM unknown_places u
         WHERE lat BETWEEN ?1 AND ?2 AND lng BETWEEN ?3 AND ?4
           AND NOT EXISTS (SELECT 1 FROM no_cuisine_reports WHERE place_id = u.id AND user_id = ?5)
           AND (SELECT COUNT(*) FROM no_cuisine_reports WHERE place_id = u.id) < 2
           AND NOT EXISTS (SELECT 1 FROM places WHERE suggested_for = u.id AND added_by = ?5)",
    )
    .bind(b.south)
    .bind(b.north)
    .bind(b.west)
    .bind(b.east)
    .bind(user)
    .fetch_all(db)
    .await?;
    Ok(rows.iter().map(unknown_place).collect())
}

/// One restaurant with unknown food.
pub async fn unknown(db: &SqlitePool, id: &str) -> sqlx::Result<Option<UnknownPlace>> {
    let row = sqlx::query("SELECT * FROM unknown_places WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await?;
    Ok(row.as_ref().map(unknown_place))
}

/// Records that `user` says a place serves no particular country's food.
pub async fn report_no_cuisine(
    db: &SqlitePool,
    user: &str,
    place_id: &str,
    at: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO no_cuisine_reports (place_id, user_id, reported_at) VALUES (?, ?, ?)",
    )
    .bind(place_id)
    .bind(user)
    .bind(at)
    .execute(db)
    .await?;
    Ok(())
}

/// Imported places inside a box.
pub async fn imported_in(db: &SqlitePool, b: Bbox) -> sqlx::Result<Vec<FoundPlace>> {
    let rows =
        sqlx::query("SELECT * FROM osm_places WHERE lat BETWEEN ? AND ? AND lng BETWEEN ? AND ?")
            .bind(b.south)
            .bind(b.north)
            .bind(b.west)
            .bind(b.east)
            .fetch_all(db)
            .await?;
    Ok(rows.iter().filter_map(found).collect())
}

#[cfg(test)]
mod tests;
