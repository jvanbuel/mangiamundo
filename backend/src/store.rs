//! SQLite persistence and the area cache.

use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::game::{Lifetime, PlaceRow};
use crate::geo::Shape;
use crate::providers::{AreaInfo, AreaKind, Boundary, FoundPlace};

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub async fn connect(url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true);
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
/// out of the result stay in `places` so existing visits are never lost.
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
    sqlx::query("DELETE FROM area_places WHERE area_id = ?").bind(&info.id).execute(&mut *tx).await?;

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

pub async fn area_rows(db: &SqlitePool, area_id: &str, user: Option<&str>) -> sqlx::Result<Vec<PlaceRow>> {
    let rows = sqlx::query(
        "SELECT pc.place_id, pc.country, v.place_id IS NOT NULL AS visited
         FROM area_places ap
         JOIN place_countries pc ON pc.place_id = ap.place_id
         LEFT JOIN visits v ON v.place_id = ap.place_id AND v.user_id = ?
         WHERE ap.area_id = ?",
    )
    .bind(user)
    .bind(area_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| PlaceRow { place_id: r.get("place_id"), country: r.get("country"), visited: r.get("visited") })
        .collect())
}

pub async fn lifetime(db: &SqlitePool, user: &str) -> sqlx::Result<Lifetime> {
    let visits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM visits WHERE user_id = ?")
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

#[derive(Debug, Serialize)]
pub struct PlaceView {
    pub id: String,
    pub name: String,
    pub lat: f64,
    pub lng: f64,
    pub address: Option<String>,
    pub url: Option<String>,
    pub cuisine: Option<String>,
    /// Raw OSM `opening_hours`.
    pub opening_hours: Option<String>,
    /// `None` when the hours are unknown.
    pub open_now: Option<bool>,
    /// Short status line, e.g. "Open until 22:00".
    pub hours_text: Option<String>,
    pub countries: Vec<String>,
    pub distance_m: u32,
    pub visited_at: Option<i64>,
    pub verified: bool,
}

/// Places of an area, optionally restricted to one country. `distance_m`, `open_now`
/// and `hours_text` are left empty.
pub async fn area_places(
    db: &SqlitePool,
    area_id: &str,
    country: Option<&str>,
    user: Option<&str>,
) -> sqlx::Result<Vec<PlaceView>> {
    let rows = sqlx::query(
        "SELECT p.id, p.name, p.lat, p.lng, p.address, p.url, p.cuisine, p.opening_hours,
                (SELECT group_concat(country) FROM place_countries WHERE place_id = p.id) AS countries,
                v.visited_at, COALESCE(v.verified, 0) AS verified
         FROM area_places ap
         JOIN places p ON p.id = ap.place_id
         LEFT JOIN visits v ON v.place_id = p.id AND v.user_id = ?1
         WHERE ap.area_id = ?2
           AND (?3 IS NULL OR EXISTS (
                SELECT 1 FROM place_countries WHERE place_id = p.id AND country = ?3))",
    )
    .bind(user)
    .bind(area_id)
    .bind(country)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| PlaceView {
            id: r.get("id"),
            name: r.get("name"),
            lat: r.get("lat"),
            lng: r.get("lng"),
            address: r.get("address"),
            url: r.get("url"),
            cuisine: r.get("cuisine"),
            opening_hours: r.get("opening_hours"),
            open_now: None,
            hours_text: None,
            countries: r
                .get::<Option<String>, _>("countries")
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_default(),
            distance_m: 0,
            visited_at: r.get("visited_at"),
            verified: r.get("verified"),
        })
        .collect())
}

/// Different places in `countries` the user checked in at between two instants.
pub async fn visits_between(
    db: &SqlitePool,
    user: &str,
    countries: &[String],
    from: i64,
    until: i64,
) -> sqlx::Result<u32> {
    if countries.is_empty() {
        return Ok(0);
    }
    let marks = vec!["?"; countries.len()].join(",");
    let sql = format!(
        "SELECT COUNT(DISTINCT v.place_id) FROM visits v
         JOIN place_countries pc ON pc.place_id = v.place_id
         WHERE v.user_id = ? AND v.visited_at >= ? AND v.visited_at < ? AND pc.country IN ({marks})"
    );
    let mut query = sqlx::query_scalar::<_, i64>(&sql).bind(user).bind(from).bind(until);
    for country in countries {
        query = query.bind(country);
    }
    Ok(query.fetch_one(db).await? as u32)
}

pub async fn place_location(db: &SqlitePool, place_id: &str) -> sqlx::Result<Option<(f64, f64)>> {
    let row = sqlx::query("SELECT lat, lng FROM places WHERE id = ?").bind(place_id).fetch_optional(db).await?;
    Ok(row.map(|r| (r.get("lat"), r.get("lng"))))
}

/// Records a visit. A repeated check-in keeps the original date but can upgrade an
/// unverified visit to a verified one.
pub async fn add_visit(db: &SqlitePool, user: &str, place_id: &str, verified: bool, at: i64) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO visits (user_id, place_id, visited_at, verified) VALUES (?, ?, ?, ?)
         ON CONFLICT(user_id, place_id) DO UPDATE SET verified = MAX(verified, excluded.verified)",
    )
    .bind(user)
    .bind(place_id)
    .bind(at)
    .bind(verified)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn remove_visit(db: &SqlitePool, user: &str, place_id: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM visits WHERE user_id = ? AND place_id = ?")
        .bind(user)
        .bind(place_id)
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
