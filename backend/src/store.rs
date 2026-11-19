//! SQLite persistence and the area cache.

use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::catalog;
use crate::game::{Lifetime, PlaceRow};
use crate::geo::{Bbox, Shape};
use crate::providers::{AreaInfo, AreaKind, Boundary, FoundPlace};

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub async fn connect(url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    // An in-memory database exists per connection, so it must never get a second one.
    let max = if url.contains(":memory:") { 1 } else { 5 };
    let pool = SqlitePoolOptions::new().max_connections(max).connect_with(options).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

/// A cached provider lookup for a city or postal code.
#[derive(Debug, Clone)]
pub struct Area {
    pub boundary: Boundary,
    pub provider: String,
    pub fetched_at: i64,
}

impl Area {
    pub fn id(&self) -> &str {
        &self.boundary.info.id
    }
}

fn kind_name(kind: AreaKind) -> &'static str {
    match kind {
        AreaKind::City => "city",
        AreaKind::PostalCode => "postal_code",
    }
}

pub async fn find_area(db: &SqlitePool, id: &str) -> sqlx::Result<Option<Area>> {
    let row = sqlx::query("SELECT * FROM areas WHERE id = ?").bind(id).fetch_optional(db).await?;
    Ok(row.and_then(|r| {
        let kind = match r.get::<String, _>("kind").as_str() {
            "city" => AreaKind::City,
            "postal_code" => AreaKind::PostalCode,
            _ => return None,
        };
        let shape: Shape = serde_json::from_str(r.get("shape")).ok()?;
        Some(Area {
            boundary: Boundary {
                info: AreaInfo {
                    id: r.get("id"),
                    name: r.get("name"),
                    kind,
                    detail: r.get("detail"),
                    lat: r.get("lat"),
                    lng: r.get("lng"),
                },
                shape,
            },
            provider: r.get("provider"),
            fetched_at: r.get("fetched_at"),
        })
    }))
}

/// Replaces the contents of an area with a fresh provider result. Places that dropped
/// out of the result stay in `places` so existing visits are never lost, and places users
/// added stay in the area.
pub async fn save_area(db: &SqlitePool, area: &Area, places: &[FoundPlace]) -> sqlx::Result<()> {
    let info = &area.boundary.info;
    let shape = serde_json::to_string(&area.boundary.shape).expect("a shape serializes");
    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT INTO areas (id, name, kind, detail, lat, lng, shape, provider, fetched_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, kind = excluded.kind,
             detail = excluded.detail, lat = excluded.lat, lng = excluded.lng,
             shape = excluded.shape, fetched_at = excluded.fetched_at",
    )
    .bind(&info.id)
    .bind(&info.name)
    .bind(kind_name(info.kind))
    .bind(&info.detail)
    .bind(info.lat)
    .bind(info.lng)
    .bind(shape)
    .bind(&area.provider)
    .bind(area.fetched_at)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM area_places WHERE area_id = ?
         AND place_id NOT IN (SELECT id FROM places WHERE added_by IS NOT NULL)",
    )
    .bind(&info.id)
    .execute(&mut *tx)
    .await?;
    // An area cached after a place was added still needs it.
    let b = area.boundary.shape.bbox();
    let added = sqlx::query(
        "SELECT id, lat, lng FROM places WHERE added_by IS NOT NULL AND lat BETWEEN ? AND ? AND lng BETWEEN ? AND ?",
    )
    .bind(b.south)
    .bind(b.north)
    .bind(b.west)
    .bind(b.east)
    .fetch_all(&mut *tx)
    .await?;
    for r in added.iter().filter(|r| area.boundary.shape.contains(r.get("lat"), r.get("lng"))) {
        sqlx::query("INSERT OR IGNORE INTO area_places (area_id, place_id) VALUES (?, ?)")
            .bind(&info.id)
            .bind(r.get::<String, _>("id"))
            .execute(&mut *tx)
            .await?;
    }

    for place in places {
        sqlx::query(
            "INSERT INTO places (id, name, lat, lng, address, url, cuisine, opening_hours, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, lat = excluded.lat,
                 lng = excluded.lng, address = excluded.address, url = excluded.url,
                 cuisine = excluded.cuisine, opening_hours = excluded.opening_hours,
                 updated_at = excluded.updated_at",
        )
        .bind(&place.id)
        .bind(&place.name)
        .bind(place.lat)
        .bind(place.lng)
        .bind(&place.address)
        .bind(&place.url)
        .bind(&place.cuisine)
        .bind(&place.opening_hours)
        .bind(area.fetched_at)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM place_countries WHERE place_id = ?").bind(&place.id).execute(&mut *tx).await?;
        for country in &place.countries {
            sqlx::query("INSERT OR IGNORE INTO place_countries (place_id, country) VALUES (?, ?)")
                .bind(&place.id)
                .bind(country)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("INSERT OR IGNORE INTO area_places (area_id, place_id) VALUES (?, ?)")
            .bind(&info.id)
            .bind(&place.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
}

pub async fn create_user(db: &SqlitePool) -> sqlx::Result<String> {
    let token = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    sqlx::query("INSERT INTO users (id, token, created_at) VALUES (?, ?, ?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&token)
        .bind(now())
        .execute(db)
        .await?;
    Ok(token)
}

pub async fn user_for_token(db: &SqlitePool, token: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT id FROM users WHERE token = ?").bind(token).fetch_optional(db).await
}

/// Hides other users' added places, places the user reported closed, and places two or
/// more users reported closed.
const VISIBLE: &str = "(p.added_by IS NULL OR p.added_by = ?1)
           AND NOT EXISTS (SELECT 1 FROM closed_reports WHERE place_id = p.id AND user_id = ?1)
           AND (SELECT COUNT(*) FROM closed_reports WHERE place_id = p.id) < 2";

pub async fn area_rows(db: &SqlitePool, area_id: &str, user: Option<&str>) -> sqlx::Result<Vec<PlaceRow>> {
    let sql = format!(
        "SELECT pc.place_id, pc.country,
                EXISTS (SELECT 1 FROM visits WHERE user_id = ?1 AND place_id = p.id) AS visited
         FROM area_places ap
         JOIN places p ON p.id = ap.place_id
         JOIN place_countries pc ON pc.place_id = p.id
         WHERE ap.area_id = ?2 AND {VISIBLE}"
    );
    let rows = sqlx::query(&sql)
    .bind(user)
    .bind(area_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| PlaceRow { place_id: r.get("place_id"), country: r.get("country"), visited: r.get("visited") })
        .collect())
}

/// Visits count each place once, however often the user went back.
pub async fn lifetime(db: &SqlitePool, user: &str) -> sqlx::Result<Lifetime> {
    let visits: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT place_id) FROM visits WHERE user_id = ?")
        .bind(user)
        .fetch_one(db)
        .await?;
    let countries: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT pc.country) FROM visits v
         JOIN place_countries pc ON pc.place_id = v.place_id WHERE v.user_id = ?",
    )
    .bind(user)
    .fetch_one(db)
    .await?;
    Ok(Lifetime { visits: visits as u32, countries: countries as u32 })
}

/// One check-in.
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct Visit {
    pub id: i64,
    /// When the user checked in (unix seconds).
    pub at: i64,
    /// Checked in within walking distance of the place.
    pub verified: bool,
}

/// Opening hours.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Hours {
    /// OSM `opening_hours` syntax; Google hours are converted to it.
    pub raw: String,
    /// `None` when `raw` can't be parsed.
    #[schema(required = true)]
    pub status: Option<HoursStatus>,
    /// This week, Monday to Sunday. Filled in only by `/api/places/{place_id}`. Lists, and
    /// hours that can't be parsed, send `null`.
    #[schema(required = true)]
    pub week: Option<Vec<crate::hours::Day>>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct HoursStatus {
    pub open: bool,
    /// Short status line, e.g. "Open until 22:00".
    pub text: String,
}

/// Every `Option` field is sent, as `null` when empty.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PlaceView {
    pub id: String,
    pub name: String,
    pub lat: f64,
    pub lng: f64,
    #[schema(required = true)]
    pub address: Option<String>,
    #[schema(required = true)]
    pub url: Option<String>,
    #[schema(required = true)]
    pub cuisine: Option<String>,
    /// `None` when the hours are unknown.
    #[schema(required = true)]
    pub hours: Option<Hours>,
    pub countries: Vec<String>,
    pub distance_m: u32,
    /// The user's visits, newest first. Lists of places carry only the latest one;
    /// `/api/places/{place_id}` carries all of them.
    pub visits: Vec<Visit>,
    /// The place's page on openstreetmap.org, for fixing it there. `None` for places that
    /// don't come from OpenStreetMap.
    #[schema(required = true)]
    pub osm_url: Option<String>,
}

const PLACE_COLUMNS: &str = "p.id, p.name, p.lat, p.lng, p.address, p.url, p.cuisine, p.opening_hours,
    (SELECT group_concat(country) FROM place_countries WHERE place_id = p.id) AS countries";

/// A place from a row with [`PLACE_COLUMNS`], without status, distance or visits.
fn place_view(r: &sqlx::sqlite::SqliteRow) -> PlaceView {
    PlaceView {
        id: r.get("id"),
        name: r.get("name"),
        lat: r.get("lat"),
        lng: r.get("lng"),
        address: r.get("address"),
        url: r.get("url"),
        cuisine: r.get("cuisine"),
        hours: r.get::<Option<String>, _>("opening_hours").map(|raw| Hours { raw, status: None, week: None }),
        countries: r
            .get::<Option<String>, _>("countries")
            .map(|s| s.split(',').map(str::to_string).collect())
            .unwrap_or_default(),
        distance_m: 0,
        visits: Vec::new(),
        osm_url: None,
    }
}

/// Places of an area, optionally restricted to one country, each with the user's latest
/// visit. `distance_m` and the hours status are left empty.
pub async fn area_places(
    db: &SqlitePool,
    area_id: &str,
    country: Option<&str>,
    user: Option<&str>,
) -> sqlx::Result<Vec<PlaceView>> {
    let sql = format!(
        "SELECT {PLACE_COLUMNS}, v.id AS visit_id, v.visited_at, v.verified
         FROM area_places ap
         JOIN places p ON p.id = ap.place_id
         LEFT JOIN visits v ON v.id = (
             SELECT id FROM visits WHERE user_id = ?1 AND place_id = p.id
             ORDER BY visited_at DESC, id DESC LIMIT 1)
         WHERE ap.area_id = ?2
           AND (?3 IS NULL OR EXISTS (
                SELECT 1 FROM place_countries WHERE place_id = p.id AND country = ?3))
           AND {VISIBLE}"
    );
    let rows = sqlx::query(&sql)
    .bind(user)
    .bind(area_id)
    .bind(country)
    .fetch_all(db)
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            let mut place = place_view(r);
            if let Some(id) = r.get::<Option<i64>, _>("visit_id") {
                place.visits.push(Visit { id, at: r.get("visited_at"), verified: r.get("verified") });
            }
            place
        })
        .collect())
}

/// Places added by users are pending: only whoever added them can see them.
const NOT_SOMEONE_ELSES: &str = "(p.added_by IS NULL OR p.added_by = ?)";

/// One place with all of the user's visits, whatever area it is in. `distance_m` and the
/// hours status are left empty.
pub async fn place(db: &SqlitePool, place_id: &str, user: Option<&str>) -> sqlx::Result<Option<PlaceView>> {
    let sql = format!("SELECT {PLACE_COLUMNS} FROM places p WHERE p.id = ? AND {NOT_SOMEONE_ELSES}");
    let Some(row) = sqlx::query(&sql).bind(place_id).bind(user).fetch_optional(db).await? else {
        return Ok(None);
    };
    let mut place = place_view(&row);
    if let Some(user) = user {
        place.visits = visits(db, user, place_id).await?;
    }
    Ok(Some(place))
}

/// The user's visits to a place, newest first.
pub async fn visits(db: &SqlitePool, user: &str, place_id: &str) -> sqlx::Result<Vec<Visit>> {
    sqlx::query_as(
        "SELECT id, visited_at AS at, verified FROM visits WHERE user_id = ? AND place_id = ?
         ORDER BY visited_at DESC, id DESC",
    )
    .bind(user)
    .bind(place_id)
    .fetch_all(db)
    .await
}

/// Different places in `countries` the user checked in at on the local days `first` to
/// `last`, inclusive. Each visit is judged by its own local date, stored at check-in.
pub async fn visits_between(
    db: &SqlitePool,
    user: &str,
    countries: &[String],
    first: NaiveDate,
    last: NaiveDate,
) -> sqlx::Result<u32> {
    if countries.is_empty() {
        return Ok(0);
    }
    let marks = vec!["?"; countries.len()].join(",");
    let sql = format!(
        "SELECT COUNT(DISTINCT v.place_id) FROM visits v
         JOIN place_countries pc ON pc.place_id = v.place_id
         WHERE v.user_id = ? AND v.local_date BETWEEN ? AND ? AND pc.country IN ({marks})"
    );
    let mut query = sqlx::query_scalar::<_, i64>(&sql).bind(user).bind(first.to_string()).bind(last.to_string());
    for country in countries {
        query = query.bind(country);
    }
    Ok(query.fetch_one(db).await? as u32)
}

/// Records a visit and returns it as stored. There is at most one visit per place per local
/// day: checking in again that day returns the same visit, verified once any of the day's
/// check-ins was made on the spot. `local_date` is the day of `at` in the user's time zone.
pub async fn add_visit(
    db: &SqlitePool,
    user: &str,
    place_id: &str,
    verified: bool,
    at: i64,
    local_date: NaiveDate,
) -> sqlx::Result<Visit> {
    sqlx::query_as(
        "INSERT INTO visits (user_id, place_id, visited_at, verified, local_date) VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, place_id, local_date) DO UPDATE SET verified = MAX(verified, excluded.verified)
         RETURNING id, visited_at AS at, verified",
    )
    .bind(user)
    .bind(place_id)
    .bind(at)
    .bind(verified)
    .bind(local_date.to_string())
    .fetch_one(db)
    .await
}

/// Removes one of the user's visits. Visits of other users are left alone.
pub async fn remove_visit(db: &SqlitePool, user: &str, visit_id: i64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM visits WHERE user_id = ? AND id = ?")
        .bind(user)
        .bind(visit_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn setting(db: &SqlitePool, key: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT value FROM settings WHERE key = ?").bind(key).fetch_optional(db).await
}

pub async fn set_setting(db: &SqlitePool, key: &str, value: &str) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(key)
        .bind(value)
        .execute(db)
        .await?;
    Ok(())
}

/// Adds a browser subscription, or moves an existing one to a new user, area or time zone.
pub async fn save_push_subscription(db: &SqlitePool, user: &str, sub: &crate::push::Subscription, at: i64) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO push_subscriptions (endpoint, user_id, p256dh, auth, area_id, timezone, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(endpoint) DO UPDATE SET user_id = excluded.user_id, p256dh = excluded.p256dh,
             auth = excluded.auth, area_id = excluded.area_id, timezone = excluded.timezone",
    )
    .bind(&sub.endpoint)
    .bind(user)
    .bind(&sub.p256dh)
    .bind(&sub.auth)
    .bind(&sub.area_id)
    .bind(&sub.timezone)
    .bind(at)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn remove_push_subscription(db: &SqlitePool, endpoint: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = ?").bind(endpoint).execute(db).await?;
    Ok(())
}

/// Removes a subscription only when it belongs to `user`. Returns whether one was removed.
pub async fn remove_user_push_subscription(db: &SqlitePool, user: &str, endpoint: &str) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = ? AND user_id = ?")
        .bind(endpoint)
        .bind(user)
        .execute(db)
        .await?;
    Ok(done.rows_affected() > 0)
}

pub async fn push_subscriptions(db: &SqlitePool) -> sqlx::Result<Vec<(String, crate::push::Subscription)>> {
    let rows = sqlx::query("SELECT * FROM push_subscriptions").fetch_all(db).await?;
    Ok(rows.iter().map(crate::push::subscription_from_row).collect())
}

/// Records that a reminder is being sent. False when it was sent before.
pub async fn mark_push_sent(db: &SqlitePool, endpoint: &str, key: &str, at: i64) -> sqlx::Result<bool> {
    let done = sqlx::query("INSERT OR IGNORE INTO push_sent (endpoint, key, sent_at) VALUES (?, ?, ?)")
        .bind(endpoint)
        .bind(key)
        .bind(at)
        .execute(db)
        .await?;
    Ok(done.rows_affected() > 0)
}

pub async fn unmark_push_sent(db: &SqlitePool, endpoint: &str, key: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM push_sent WHERE endpoint = ? AND key = ?").bind(endpoint).bind(key).execute(db).await?;
    Ok(())
}

/// Records that `user` says a place has closed. `None` when they had already said so,
/// otherwise how many users have now reported it.
pub async fn report_closed(db: &SqlitePool, user: &str, place_id: &str, at: i64) -> sqlx::Result<Option<i64>> {
    let done = sqlx::query("INSERT OR IGNORE INTO closed_reports (place_id, user_id, reported_at) VALUES (?, ?, ?)")
        .bind(place_id)
        .bind(user)
        .bind(at)
        .execute(db)
        .await?;
    if done.rows_affected() == 0 {
        return Ok(None);
    }
    let count = sqlx::query_scalar("SELECT COUNT(*) FROM closed_reports WHERE place_id = ?")
        .bind(place_id)
        .fetch_one(db)
        .await?;
    Ok(Some(count))
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
}

/// Saves a place added by `user` and puts it in the cached areas around it. Returns its id,
/// or `None` when the user already added `limit` places in the day before `at`. Checking and
/// inserting is one statement, so concurrent requests can't both take the last slot.
pub async fn add_place(db: &SqlitePool, user: &str, place: &NewPlace, at: i64, limit: i64) -> sqlx::Result<Option<String>> {
    let id = format!("user-{}", uuid::Uuid::new_v4().simple());
    let mut tx = db.begin().await?;
    let added = sqlx::query(
        "INSERT INTO places (id, name, lat, lng, address, url, cuisine, updated_at, added_by, added_at)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?8
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
    // Areas are at most 1.5° across, so any area holding the place is centred within that.
    let near = sqlx::query("SELECT id, shape FROM areas WHERE lat BETWEEN ? AND ? AND lng BETWEEN ? AND ?")
        .bind(place.lat - 1.5)
        .bind(place.lat + 1.5)
        .bind(place.lng - 1.5)
        .bind(place.lng + 1.5)
        .fetch_all(&mut *tx)
        .await?;
    for r in near {
        let inside = serde_json::from_str::<Shape>(r.get("shape")).is_ok_and(|s| s.contains(place.lat, place.lng));
        if inside {
            sqlx::query("INSERT INTO area_places (area_id, place_id) VALUES (?, ?)")
                .bind(r.get::<String, _>("id"))
                .bind(&id)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    Ok(Some(id))
}

pub async fn set_osm_note(db: &SqlitePool, place_id: &str, note_id: i64) -> sqlx::Result<()> {
    sqlx::query("UPDATE places SET osm_note_id = ? WHERE id = ?").bind(note_id).bind(place_id).execute(db).await?;
    Ok(())
}

/// Name and location of a place `user` can see.
pub async fn place_summary(db: &SqlitePool, id: &str, user: &str) -> sqlx::Result<Option<(String, f64, f64)>> {
    let sql = format!("SELECT name, lat, lng FROM places p WHERE id = ? AND {NOT_SOMEONE_ELSES}");
    let row = sqlx::query(&sql).bind(id).bind(user).fetch_optional(db).await?;
    Ok(row.map(|r| (r.get("name"), r.get("lat"), r.get("lng"))))
}

/// Replaces the imported OpenStreetMap places in `bbox`, and marks the cached areas centred
/// in it as stale so their next request rebuilds them from the new copy.
pub async fn replace_imported(db: &SqlitePool, bbox: Bbox, places: &[FoundPlace]) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    // ponytail: by centre, so an area straddling the box edge is refetched live once per import.
    sqlx::query("UPDATE areas SET fetched_at = 0 WHERE lat BETWEEN ? AND ? AND lng BETWEEN ? AND ?")
        .bind(bbox.south)
        .bind(bbox.north)
        .bind(bbox.west)
        .bind(bbox.east)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM osm_places").execute(&mut *tx).await?;
    for place in places {
        let Some(cuisine) = &place.cuisine else { continue };
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
    }
    tx.commit().await
}

/// Imported places inside a box.
pub async fn imported_in(db: &SqlitePool, b: Bbox) -> sqlx::Result<Vec<FoundPlace>> {
    let rows = sqlx::query(
        "SELECT * FROM osm_places WHERE lat BETWEEN ? AND ? AND lng BETWEEN ? AND ?",
    )
    .bind(b.south)
    .bind(b.north)
    .bind(b.west)
    .bind(b.east)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
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
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(id: &str, lat: f64, lng: f64, cuisine: &str) -> FoundPlace {
        FoundPlace {
            id: id.into(),
            name: id.into(),
            lat,
            lng,
            address: None,
            url: None,
            cuisine: Some(cuisine.into()),
            opening_hours: None,
            countries: vec![],
        }
    }

    #[tokio::test]
    async fn imported_places_are_found_inside_a_box() {
        let db = connect("sqlite::memory:").await.unwrap();
        let all = Bbox { south: -90.0, west: -180.0, north: 90.0, east: 180.0 };
        replace_imported(&db, all, &[place("in", 50.88, 4.70, "greek"), place("out", 51.2, 4.4, "greek"), place("odd", 50.88, 4.70, "burger")])
            .await
            .unwrap();
        let found = imported_in(&db, Bbox { south: 50.8, west: 4.6, north: 50.9, east: 4.8 }).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "in");
        assert_eq!(found[0].countries, vec!["GR"]);
    }

    #[tokio::test]
    async fn an_import_marks_the_areas_in_its_box_stale() {
        let db = connect("sqlite::memory:").await.unwrap();
        for (id, lat, lng) in [("leuven", 50.88, 4.70), ("paris", 48.86, 2.35)] {
            sqlx::query("INSERT INTO areas (id, name, kind, lat, lng, shape, provider, fetched_at) VALUES (?, ?, 'city', ?, ?, '[]', 'osm', 1000)")
                .bind(id)
                .bind(id)
                .bind(lat)
                .bind(lng)
                .execute(&db)
                .await
                .unwrap();
        }
        replace_imported(&db, Bbox { south: 49.4, west: 2.4, north: 51.6, east: 6.5 }, &[]).await.unwrap();
        let fetched: Vec<(String, i64)> =
            sqlx::query_as("SELECT id, fetched_at FROM areas ORDER BY id").fetch_all(&db).await.unwrap();
        assert_eq!(fetched, vec![("leuven".into(), 0), ("paris".into(), 1000)]);
    }
}
