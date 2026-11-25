//! The restaurants a user added, as they see them.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::game::Bonus;

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
