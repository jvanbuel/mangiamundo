//! SQLite persistence and the area cache.

use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::game::{Lifetime, PlaceRow};
use crate::providers::FoundPlace;

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

/// A cached provider lookup around a grid-cell centre.
#[derive(Debug, Clone, Serialize)]
pub struct Area {
    #[serde(skip)]
    pub key: String,
    pub lat: f64,
    pub lng: f64,
    pub radius_m: u32,
    pub provider: String,
    pub fetched_at: i64,
}

/// Nearby requests share one cache entry: coordinates snap to a 0.02° grid (about 2 km).
const GRID: f64 = 0.02;

pub fn snap(lat: f64, lng: f64) -> (f64, f64) {
    let snap = |v: f64| ((v / GRID).round() * GRID * 100.0).round() / 100.0;
    (snap(lat), snap(lng))
}

pub fn area_key(provider: &str, lat: f64, lng: f64, radius_m: u32) -> String {
    format!("{provider}:{lat:.2}:{lng:.2}:{radius_m}")
}

pub async fn find_area(db: &SqlitePool, key: &str) -> sqlx::Result<Option<Area>> {
    let row = sqlx::query("SELECT key, lat, lng, radius_m, provider, fetched_at FROM areas WHERE key = ?")
        .bind(key)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|r| Area {
        key: r.get("key"),
        lat: r.get("lat"),
        lng: r.get("lng"),
        radius_m: r.get::<i64, _>("radius_m") as u32,
        provider: r.get("provider"),
        fetched_at: r.get("fetched_at"),
    }))
}

/// Replaces the contents of an area with a fresh provider result. Places that dropped
/// out of the result stay in `places` so existing visits are never lost.
pub async fn save_area(db: &SqlitePool, area: &Area, places: &[FoundPlace]) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT INTO areas (key, lat, lng, radius_m, provider, fetched_at) VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET fetched_at = excluded.fetched_at",
    )
    .bind(&area.key)
    .bind(area.lat)
    .bind(area.lng)
    .bind(area.radius_m)
    .bind(&area.provider)
    .bind(area.fetched_at)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM area_places WHERE area_key = ?").bind(&area.key).execute(&mut *tx).await?;

    for place in places {
        sqlx::query(
            "INSERT INTO places (id, name, lat, lng, address, url, cuisine, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, lat = excluded.lat,
                 lng = excluded.lng, address = excluded.address, url = excluded.url,
                 cuisine = excluded.cuisine, updated_at = excluded.updated_at",
        )
        .bind(&place.id)
        .bind(&place.name)
        .bind(place.lat)
        .bind(place.lng)
        .bind(&place.address)
        .bind(&place.url)
        .bind(&place.cuisine)
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
        sqlx::query("INSERT OR IGNORE INTO area_places (area_key, place_id) VALUES (?, ?)")
            .bind(&area.key)
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

pub async fn area_rows(db: &SqlitePool, area_key: &str, user: Option<&str>) -> sqlx::Result<Vec<PlaceRow>> {
    let rows = sqlx::query(
        "SELECT pc.place_id, pc.country, v.place_id IS NOT NULL AS visited
         FROM area_places ap
         JOIN place_countries pc ON pc.place_id = ap.place_id
         LEFT JOIN visits v ON v.place_id = ap.place_id AND v.user_id = ?
         WHERE ap.area_key = ?",
    )
    .bind(user)
    .bind(area_key)
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
    pub countries: Vec<String>,
    pub distance_m: u32,
    pub visited_at: Option<i64>,
    pub verified: bool,
}

/// Places of an area, optionally restricted to one country. `distance_m` is left at 0.
pub async fn area_places(
    db: &SqlitePool,
    area_key: &str,
    country: Option<&str>,
    user: Option<&str>,
) -> sqlx::Result<Vec<PlaceView>> {
    let rows = sqlx::query(
        "SELECT p.id, p.name, p.lat, p.lng, p.address, p.url, p.cuisine,
                (SELECT group_concat(country) FROM place_countries WHERE place_id = p.id) AS countries,
                v.visited_at, COALESCE(v.verified, 0) AS verified
         FROM area_places ap
         JOIN places p ON p.id = ap.place_id
         LEFT JOIN visits v ON v.place_id = p.id AND v.user_id = ?1
         WHERE ap.area_key = ?2
           AND (?3 IS NULL OR EXISTS (
                SELECT 1 FROM place_countries WHERE place_id = p.id AND country = ?3))",
    )
    .bind(user)
    .bind(area_key)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearby_points_share_a_cell() {
        assert_eq!(snap(50.8798, 4.7005), (50.88, 4.7));
        assert_eq!(snap(50.8840, 4.7090), (50.88, 4.7));
        assert_ne!(snap(50.8798, 4.7005), snap(50.92, 4.7005));
        assert_eq!(area_key("osm", 50.88, 4.7, 10_000), "osm:50.88:4.70:10000");
    }
}
