//! Places as clients see them, and closed reports.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use super::areas::upsert_place;
use super::{imported, now, visits, Visit};
use crate::catalog;
use crate::providers::FoundPlace;

/// Places added by users are pending: only whoever added them can see them. The user is
/// parameter `?1`.
pub(super) const NOT_SOMEONE_ELSES: &str = "(p.added_by IS NULL OR p.added_by = ?1)";

/// Hides other users' added places, places the user reported closed, and places two or
/// more users reported closed. The user is parameter `?1`.
pub(super) fn visible() -> String {
    format!(
        "{NOT_SOMEONE_ELSES}
           AND NOT EXISTS (SELECT 1 FROM closed_reports WHERE place_id = p.id AND user_id = ?1)
           AND (SELECT COUNT(*) FROM closed_reports WHERE place_id = p.id) < 2"
    )
}

/// Opening hours.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Hours {
    /// OSM `opening_hours` syntax.
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

const PLACE_COLUMNS: &str =
    "p.id, p.name, p.lat, p.lng, p.address, p.url, p.cuisine, p.opening_hours,
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
        hours: r
            .get::<Option<String>, _>("opening_hours")
            .map(|raw| Hours {
                raw,
                status: None,
                week: None,
            }),
        countries: r
            .get::<Option<String>, _>("countries")
            .map_or_default(|s| s.split(',').map(str::to_string).collect()),
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
         {LATEST_VISIT}
         WHERE ap.area_id = ?2
           AND (?3 IS NULL OR EXISTS (
                SELECT 1 FROM place_countries WHERE place_id = p.id AND country = ?3))
           AND {visible}",
        visible = visible()
    );
    let rows = sqlx::query(&sql)
        .bind(user)
        .bind(area_id)
        .bind(country)
        .fetch_all(db)
        .await?;
    Ok(rows.iter().map(with_latest_visit).collect())
}

/// Joins the user's latest visit to a place, as `v`. The user is parameter `?1`.
const LATEST_VISIT: &str = "LEFT JOIN visits v ON v.id = (
             SELECT id FROM visits WHERE user_id = ?1 AND place_id = p.id
             ORDER BY visited_at DESC, id DESC LIMIT 1)";

/// A place from a row with [`PLACE_COLUMNS`] and the columns of [`LATEST_VISIT`].
fn with_latest_visit(r: &sqlx::sqlite::SqliteRow) -> PlaceView {
    let mut place = place_view(r);
    if let Some(id) = r.get::<Option<i64>, _>("visit_id") {
        place.visits.push(Visit {
            id,
            at: r.get("visited_at"),
            verified: r.get("verified"),
        });
    }
    place
}

/// The `limit` places serving a country's food nearest to a point, nearest first, whatever
/// area they are in, each with the user's latest visit and `distance_m` filled in. Places
/// come from the cached areas and from the local copy of OpenStreetMap; one only in the
/// copy is saved as a place first, so it can be opened and checked in at like any other.
pub async fn nearest(
    db: &SqlitePool,
    country: &str,
    user: Option<&str>,
    (lat, lng): (f64, f64),
    limit: usize,
) -> sqlx::Result<Vec<PlaceView>> {
    // Both reads and the saves below see one state: without it a place another request
    // saves in between would be in neither read. IMMEDIATE takes the write lock up front,
    // so the saves can't fail on a snapshot that went stale.
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    // Listed: in a cached area or in the copy. A place gone from OpenStreetMap left both.
    let sql = format!(
        "SELECT {PLACE_COLUMNS}, v.id AS visit_id, v.visited_at, v.verified
         FROM places p
         {LATEST_VISIT}
         WHERE EXISTS (SELECT 1 FROM place_countries WHERE place_id = p.id AND country = ?2)
           AND (EXISTS (SELECT 1 FROM area_places WHERE place_id = p.id)
                OR EXISTS (SELECT 1 FROM osm_places WHERE id = p.id))
           AND {visible}",
        visible = visible()
    );
    let rows = sqlx::query(&sql)
        .bind(user)
        .bind(country)
        .fetch_all(&mut *tx)
        .await?;
    let mut found: Vec<Nearby> = rows
        .iter()
        .map(|r| Nearby::Listed(with_latest_visit(r)))
        .collect();
    // The copy keeps raw `cuisine` values, so match their tags loosely here and exactly below.
    let tags = catalog::cuisine_tags(country);
    if !tags.is_empty() {
        let like = vec!["lower(replace(cuisine, ' ', '_')) LIKE ?"; tags.len()].join(" OR ");
        let sql = format!(
            "SELECT * FROM osm_places WHERE id NOT IN (SELECT id FROM places) AND ({like})"
        );
        let mut query = sqlx::query(&sql);
        for tag in tags {
            query = query.bind(format!("%{tag}%"));
        }
        found.extend(
            query
                .fetch_all(&mut *tx)
                .await?
                .iter()
                .filter_map(imported::found)
                .filter(|p| p.countries.contains(&country))
                .map(Nearby::Imported),
        );
    }
    found.sort_by_cached_key(|p| {
        let (plat, plng) = p.position();
        crate::geo::rounded_m(lat, lng, plat, plng)
    });
    found.truncate(limit);
    let mut nearest = Vec::with_capacity(found.len());
    for place in found {
        let mut view = match place {
            Nearby::Listed(view) => view,
            Nearby::Imported(place) => {
                upsert_place(&mut tx, &place, now()).await?;
                PlaceView {
                    id: place.id,
                    name: place.name,
                    lat: place.lat,
                    lng: place.lng,
                    address: place.address,
                    url: place.url,
                    cuisine: place.cuisine,
                    hours: place.opening_hours.map(|raw| Hours {
                        raw,
                        status: None,
                        week: None,
                    }),
                    countries: place.countries.iter().map(|c| (*c).to_string()).collect(),
                    distance_m: 0,
                    visits: Vec::new(),
                    osm_url: None,
                }
            }
        };
        view.distance_m = crate::geo::rounded_m(lat, lng, view.lat, view.lng);
        nearest.push(view);
    }
    tx.commit().await?;
    Ok(nearest)
}

/// A candidate for [`nearest`].
enum Nearby {
    Listed(PlaceView),
    /// Only in the local copy of OpenStreetMap so far.
    Imported(FoundPlace),
}

impl Nearby {
    fn position(&self) -> (f64, f64) {
        match self {
            Nearby::Listed(p) => (p.lat, p.lng),
            Nearby::Imported(p) => (p.lat, p.lng),
        }
    }
}

/// One place with all of the user's visits, whatever area it is in. `distance_m` and the
/// hours status are left empty.
pub async fn place(
    db: &SqlitePool,
    place_id: &str,
    user: Option<&str>,
) -> sqlx::Result<Option<PlaceView>> {
    let sql =
        format!("SELECT {PLACE_COLUMNS} FROM places p WHERE {NOT_SOMEONE_ELSES} AND p.id = ?2");
    let Some(row) = sqlx::query(&sql)
        .bind(user)
        .bind(place_id)
        .fetch_optional(db)
        .await?
    else {
        return Ok(None);
    };
    let mut place = place_view(&row);
    if let Some(user) = user {
        place.visits = visits(db, user, place_id).await?;
    }
    Ok(Some(place))
}

/// Records that `user` says a place has closed. `None` when they had already said so,
/// otherwise how many users have now reported it.
pub async fn report_closed(
    db: &SqlitePool,
    user: &str,
    place_id: &str,
    at: i64,
) -> sqlx::Result<Option<i64>> {
    let done = sqlx::query(
        "INSERT OR IGNORE INTO closed_reports (place_id, user_id, reported_at) VALUES (?, ?, ?)",
    )
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

/// Name and location of a place `user` can see.
pub async fn place_summary(
    db: &SqlitePool,
    id: &str,
    user: &str,
) -> sqlx::Result<Option<(String, f64, f64)>> {
    let sql = format!("SELECT name, lat, lng FROM places p WHERE {NOT_SOMEONE_ELSES} AND id = ?2");
    let row = sqlx::query(&sql)
        .bind(user)
        .bind(id)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|r| (r.get("name"), r.get("lat"), r.get("lng"))))
}
