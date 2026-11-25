//! Check-ins.

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::SqlitePool;

/// One check-in.
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct Visit {
    pub id: i64,
    /// When the user checked in (unix seconds).
    pub at: i64,
    /// Checked in within walking distance of the place.
    pub verified: bool,
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
    let mut query = sqlx::query_scalar::<_, i64>(&sql)
        .bind(user)
        .bind(first.to_string())
        .bind(last.to_string());
    for country in countries {
        query = query.bind(country);
    }
    Ok(crate::count(query.fetch_one(db).await?))
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

/// Removes one of the user's visits and returns its place and local date, if there was
/// such a visit. Visits of other users are left alone.
pub async fn remove_visit(
    db: &SqlitePool,
    user: &str,
    visit_id: i64,
) -> sqlx::Result<Option<(String, String)>> {
    sqlx::query_as("DELETE FROM visits WHERE user_id = ? AND id = ? RETURNING place_id, local_date")
        .bind(user)
        .bind(visit_id)
        .fetch_optional(db)
        .await
}

/// Every country the user has checked in at a place of.
pub async fn visited_countries(db: &SqlitePool, user: &str) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT DISTINCT pc.country FROM visits v JOIN place_countries pc ON pc.place_id = v.place_id WHERE v.user_id = ?",
    )
    .bind(user)
    .fetch_all(db)
    .await
}
