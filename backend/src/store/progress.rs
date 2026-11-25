//! What the game reads: an area's places, the user's lifetime totals and quest rewards.

use chrono::NaiveDate;
use sqlx::{Row, SqlitePool};

use super::places::visible;
use crate::game::{self, Lifetime, PlaceRow, Tasted};

pub async fn area_rows(
    db: &SqlitePool,
    area_id: &str,
    user: Option<&str>,
) -> sqlx::Result<Vec<PlaceRow>> {
    let sql = format!(
        "SELECT pc.place_id, pc.country,
                EXISTS (SELECT 1 FROM visits WHERE user_id = ?1 AND place_id = p.id) AS visited
         FROM area_places ap
         JOIN places p ON p.id = ap.place_id
         JOIN place_countries pc ON pc.place_id = p.id
         WHERE ap.area_id = ?2 AND {visible}",
        visible = visible()
    );
    let rows = sqlx::query(&sql)
        .bind(user)
        .bind(area_id)
        .fetch_all(db)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| PlaceRow {
            place_id: r.get("place_id"),
            country: r.get("country"),
            visited: r.get("visited"),
        })
        .collect())
}

/// Visits count each place once, however often the user went back. `today` is the user's
/// local date, for the streak.
pub async fn lifetime(db: &SqlitePool, user: &str, today: NaiveDate) -> sqlx::Result<Lifetime> {
    let (visits, verified): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(DISTINCT place_id), COUNT(DISTINCT place_id) FILTER (WHERE verified)
         FROM visits WHERE user_id = ?",
    )
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
    let (added, country_bonuses, continent_bonuses): (i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE bonus != 'none'), COUNT(*) FILTER (WHERE bonus = 'continent')
         FROM additions WHERE user_id = ? AND rewarded",
    )
    .bind(user)
    .fetch_one(db)
    .await?;
    let quest_xp: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(xp), 0) FROM quest_rewards WHERE user_id = ?")
            .bind(user)
            .fetch_one(db)
            .await?;
    let days: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT local_date FROM visits WHERE user_id = ?")
            .bind(user)
            .fetch_all(db)
            .await?;
    let days: Vec<NaiveDate> = days.iter().filter_map(|d| d.parse().ok()).collect();
    let (streak, best_streak) = game::streaks(&days, today);
    Ok(Lifetime {
        visits: crate::count(visits),
        verified: crate::count(verified),
        countries: crate::count(countries),
        added: crate::count(added),
        country_bonuses: crate::count(country_bonuses),
        continent_bonuses: crate::count(continent_bonuses),
        quest_xp: crate::count(quest_xp),
        streak,
        best_streak,
    })
}

/// Every place the user checked in at, any area, per country it serves, with the local days
/// of the first and latest check-in.
pub async fn tasted(db: &SqlitePool, user: &str) -> sqlx::Result<Vec<Tasted>> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT pc.place_id, pc.country, MIN(v.local_date), MAX(v.local_date) FROM visits v
         JOIN place_countries pc ON pc.place_id = v.place_id
         WHERE v.user_id = ? GROUP BY pc.place_id, pc.country",
    )
    .bind(user)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(place_id, country, first, last)| {
            Some(Tasted {
                place_id,
                country,
                first: first.parse().ok()?,
                last: last.parse().ok()?,
            })
        })
        .collect())
}

/// Keeps the XP of the quests the user finished in an area. A quest pays once per area
/// and month.
pub async fn reward_quests(
    db: &SqlitePool,
    user: &str,
    area_id: &str,
    month: &str,
    quests: &[(&str, u32)],
) -> sqlx::Result<()> {
    for (quest, xp) in quests {
        sqlx::query(
            "INSERT OR IGNORE INTO quest_rewards (user_id, area_id, month, quest, xp) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(user)
        .bind(area_id)
        .bind(month)
        .bind(quest)
        .bind(xp)
        .execute(db)
        .await?;
    }
    Ok(())
}

/// Takes back the XP of one quest in an area and month, when it is no longer finished.
pub async fn unreward_quest(
    db: &SqlitePool,
    user: &str,
    area_id: &str,
    month: &str,
    quest: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "DELETE FROM quest_rewards WHERE user_id = ? AND area_id = ? AND month = ? AND quest = ?",
    )
    .bind(user)
    .bind(area_id)
    .bind(month)
    .bind(quest)
    .execute(db)
    .await?;
    Ok(())
}
