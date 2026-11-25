//! Restaurants OpenStreetMap gives no cuisine, and what users say they serve.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::geo::Bbox;

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
