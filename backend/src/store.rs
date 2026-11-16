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

/// Crockford base32: no I, L, O or U, so a code survives being read aloud or handwritten.
const CODE_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// 16 characters of 5 bits: 80 random bits, far beyond guessing.
const CODE_LEN: usize = 16;

fn new_recovery_code() -> String {
    let mut bytes = [0u8; CODE_LEN];
    getrandom::fill(&mut bytes).expect("the OS has a random source");
    bytes.iter().map(|b| CODE_ALPHABET[(b & 31) as usize] as char).collect()
}

/// Turns what a user typed into the stored form: dashes and spaces dropped, look-alike
/// letters read as the digits they resemble. None when it cannot be a code.
pub fn normalize_recovery_code(input: &str) -> Option<String> {
    let mut out = String::new();
    for ch in input.chars() {
        let ch = match ch.to_ascii_uppercase() {
            '-' | ' ' => continue,
            'O' => '0',
            'I' | 'L' => '1',
            c if c.is_ascii() && CODE_ALPHABET.contains(&(c as u8)) => c,
            _ => return None,
        };
        out.push(ch);
    }
    (out.len() == CODE_LEN).then_some(out)
}

/// The user's recovery code, created on first request.
pub async fn recovery_code(db: &SqlitePool, user: &str) -> sqlx::Result<String> {
    let existing: Option<String> = sqlx::query_scalar("SELECT recovery_code FROM users WHERE id = ?")
        .bind(user)
        .fetch_one(db)
        .await?;
    match existing {
        Some(code) => Ok(code),
        None => rotate_recovery_code(db, user).await,
    }
}

/// Replaces the user's recovery code, so a code that was seen by someone else stops working.
pub async fn rotate_recovery_code(db: &SqlitePool, user: &str) -> sqlx::Result<String> {
    let code = new_recovery_code();
    sqlx::query("UPDATE users SET recovery_code = ? WHERE id = ?").bind(&code).bind(user).execute(db).await?;
    Ok(code)
}

/// The user id and session token behind a (normalized) recovery code.
pub async fn user_for_recovery_code(db: &SqlitePool, code: &str) -> sqlx::Result<Option<(String, String)>> {
    let row = sqlx::query("SELECT id, token FROM users WHERE recovery_code = ?").bind(code).fetch_optional(db).await?;
    Ok(row.map(|r| (r.get("id"), r.get("token"))))
}

/// Moves everything `from` has (check-ins, push subscriptions) to `into` and deletes
/// `from`. Returns how many check-ins moved.
pub async fn merge_users(db: &SqlitePool, from: &str, into: &str) -> sqlx::Result<u64> {
    let mut tx = db.begin().await?;
    let moved = sqlx::query("UPDATE checkins SET user_id = ? WHERE user_id = ?")
        .bind(into)
        .bind(from)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    sqlx::query("UPDATE push_subscriptions SET user_id = ? WHERE user_id = ?")
        .bind(into)
        .bind(from)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?").bind(from).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(moved)
}

pub async fn area_rows(db: &SqlitePool, area_key: &str, user: Option<&str>) -> sqlx::Result<Vec<PlaceRow>> {
    let rows = sqlx::query(
        "SELECT pc.place_id, pc.country,
                EXISTS (SELECT 1 FROM checkins c WHERE c.place_id = ap.place_id AND c.user_id = ?1) AS visited
         FROM area_places ap
         JOIN place_countries pc ON pc.place_id = ap.place_id
         WHERE ap.area_key = ?2",
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
    let row = sqlx::query(
        "SELECT COUNT(*) AS visits,
                COUNT(DISTINCT dish) AS dishes,
                (SELECT COUNT(DISTINCT pc.country) FROM checkins c
                 JOIN place_countries pc ON pc.place_id = c.place_id WHERE c.user_id = ?1) AS countries,
                (SELECT COALESCE(MAX(n), 0) FROM
                    (SELECT COUNT(*) AS n FROM checkins WHERE user_id = ?1 GROUP BY place_id)) AS most_at_one_place
         FROM checkins WHERE user_id = ?1",
    )
    .bind(user)
    .fetch_one(db)
    .await?;
    let count = |name: &str| r_u32(&row, name);
    Ok(Lifetime {
        visits: count("visits"),
        countries: count("countries"),
        dishes: count("dishes"),
        most_at_one_place: count("most_at_one_place"),
    })
}

fn r_u32(row: &sqlx::sqlite::SqliteRow, name: &str) -> u32 {
    row.get::<i64, _>(name).max(0) as u32
}

/// Signature dishes the user logged, ever.
pub async fn dishes_tried(db: &SqlitePool, user: &str) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT DISTINCT dish FROM checkins WHERE user_id = ? AND dish IS NOT NULL ORDER BY dish")
        .bind(user)
        .fetch_all(db)
        .await
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
    /// First check-in. Set means the place counts as visited.
    pub visited_at: Option<i64>,
    /// Whether any check-in here was made on the spot.
    pub verified: bool,
    /// Number of check-ins here.
    pub visits: u32,
    pub last_visited_at: Option<i64>,
    /// The latest check-in, so a client can log a dish on it or undo it.
    pub last_checkin_id: Option<i64>,
    pub last_dish: Option<String>,
    /// When checking in again becomes possible, if not right now. Filled in by the API.
    pub next_check_in_at: Option<i64>,
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
                v.visited_at, COALESCE(v.verified, 0) AS verified, COALESCE(v.visits, 0) AS visits,
                v.last_visited_at, last.id AS last_checkin_id, last.dish AS last_dish
         FROM area_places ap
         JOIN places p ON p.id = ap.place_id
         LEFT JOIN (
             SELECT place_id, MIN(at) AS visited_at, MAX(at) AS last_visited_at,
                    MAX(verified) AS verified, COUNT(*) AS visits
             FROM checkins WHERE user_id = ?1 GROUP BY place_id
         ) v ON v.place_id = p.id
         LEFT JOIN checkins last ON last.id = (
             SELECT id FROM checkins WHERE user_id = ?1 AND place_id = p.id ORDER BY at DESC, id DESC LIMIT 1)
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
            visits: r_u32(&r, "visits"),
            last_visited_at: r.get("last_visited_at"),
            last_checkin_id: r.get("last_checkin_id"),
            last_dish: r.get("last_dish"),
            next_check_in_at: None,
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
        "SELECT COUNT(DISTINCT c.place_id) FROM checkins c
         JOIN place_countries pc ON pc.place_id = c.place_id
         WHERE c.user_id = ? AND c.at >= ? AND c.at < ? AND pc.country IN ({marks})"
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

pub async fn place_countries(db: &SqlitePool, place_id: &str) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT country FROM place_countries WHERE place_id = ?").bind(place_id).fetch_all(db).await
}

/// When the user last checked in at a place.
pub async fn last_checkin_at(db: &SqlitePool, user: &str, place_id: &str) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar("SELECT MAX(at) FROM checkins WHERE user_id = ? AND place_id = ?")
        .bind(user)
        .bind(place_id)
        .fetch_one(db)
        .await
}

/// Records a check-in and returns its id.
pub async fn add_checkin(
    db: &SqlitePool,
    user: &str,
    place_id: &str,
    verified: bool,
    dish: Option<&str>,
    at: i64,
) -> sqlx::Result<i64> {
    let done = sqlx::query("INSERT INTO checkins (user_id, place_id, at, verified, dish) VALUES (?, ?, ?, ?, ?)")
        .bind(user)
        .bind(place_id)
        .bind(at)
        .bind(verified)
        .bind(dish)
        .execute(db)
        .await?;
    Ok(done.last_insert_rowid())
}

/// The user's latest check-in at a place. Only call when there is one.
pub async fn latest_checkin_id(db: &SqlitePool, user: &str, place_id: &str) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT id FROM checkins WHERE user_id = ? AND place_id = ? ORDER BY at DESC, id DESC LIMIT 1")
        .bind(user)
        .bind(place_id)
        .fetch_one(db)
        .await
}

/// Marks the latest check-in at a place as verified, for a repeated "I was here" that
/// should not count as a new visit.
pub async fn verify_latest_checkin(db: &SqlitePool, user: &str, place_id: &str) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE checkins SET verified = 1 WHERE id = (
             SELECT id FROM checkins WHERE user_id = ? AND place_id = ? ORDER BY at DESC, id DESC LIMIT 1)",
    )
    .bind(user)
    .bind(place_id)
    .execute(db)
    .await?;
    Ok(())
}

/// The place of one of the user's check-ins.
pub async fn checkin_place(db: &SqlitePool, user: &str, id: i64) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT place_id FROM checkins WHERE id = ? AND user_id = ?")
        .bind(id)
        .bind(user)
        .fetch_optional(db)
        .await
}

pub async fn set_checkin_dish(db: &SqlitePool, user: &str, id: i64, dish: Option<&str>) -> sqlx::Result<()> {
    sqlx::query("UPDATE checkins SET dish = ? WHERE id = ? AND user_id = ?")
        .bind(dish)
        .bind(id)
        .bind(user)
        .execute(db)
        .await?;
    Ok(())
}

/// Deletes one check-in. Returns whether it existed.
pub async fn remove_checkin(db: &SqlitePool, user: &str, id: i64) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM checkins WHERE id = ? AND user_id = ?").bind(id).bind(user).execute(db).await?;
    Ok(done.rows_affected() > 0)
}

/// Deletes every check-in of the user at a place.
pub async fn remove_visits(db: &SqlitePool, user: &str, place_id: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM checkins WHERE user_id = ? AND place_id = ?")
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

/// Adds a browser subscription, or moves an existing one to a new user, place or time zone.
pub async fn save_push_subscription(db: &SqlitePool, user: &str, sub: &crate::push::Subscription, at: i64) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO push_subscriptions (endpoint, user_id, p256dh, auth, lat, lng, radius_km, timezone, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(endpoint) DO UPDATE SET user_id = excluded.user_id, p256dh = excluded.p256dh,
             auth = excluded.auth, lat = excluded.lat, lng = excluded.lng,
             radius_km = excluded.radius_km, timezone = excluded.timezone",
    )
    .bind(&sub.endpoint)
    .bind(user)
    .bind(&sub.p256dh)
    .bind(&sub.auth)
    .bind(sub.lat)
    .bind(sub.lng)
    .bind(sub.radius_km)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_codes_are_forgiving_to_type() {
        let code = new_recovery_code();
        assert_eq!(code.len(), CODE_LEN);
        assert_eq!(normalize_recovery_code(&code).as_deref(), Some(code.as_str()));
        assert_eq!(
            normalize_recovery_code("abcd-efgh-jkmn-pqrO").as_deref(),
            Some("ABCDEFGHJKMNPQR0")
        );
        assert_eq!(normalize_recovery_code("ilIL 2345 6789 abcd").as_deref(), Some("111123456789ABCD"));
        assert_eq!(normalize_recovery_code("too-short"), None);
        assert_eq!(normalize_recovery_code("ABCD-EFGH-JKMN-PQRU"), None);
    }

    #[test]
    fn nearby_points_share_a_cell() {
        assert_eq!(snap(50.8798, 4.7005), (50.88, 4.7));
        assert_eq!(snap(50.8840, 4.7090), (50.88, 4.7));
        assert_ne!(snap(50.8798, 4.7005), snap(50.92, 4.7005));
        assert_eq!(area_key("osm", 50.88, 4.7, 10_000), "osm:50.88:4.70:10000");
    }
}
