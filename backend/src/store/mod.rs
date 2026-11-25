//! SQLite persistence and the area cache.

mod adding;
mod additions;
mod areas;
mod imported;
mod matching;
mod places;
mod progress;
mod subscriptions;
mod unknown;
mod visits;

pub use adding::{add_place, set_osm_note, AddedPlace, NewPlace};
pub use additions::{additions, mark_additions_seen, Addition, AdditionStatus};
pub use areas::{areas_containing, areas_of_place, find_area, save_area, Area};
pub use imported::{add_nearby, imported_in, refresh_place, replace_imported, Refreshed};
pub use places::{area_places, place, place_summary, report_closed, Hours, HoursStatus, PlaceView};
pub use progress::{area_rows, lifetime, reward_quests, tasted, unreward_quest};
pub use subscriptions::{
    forget_countries, mark_push_sent, push_subscriptions, remove_push_subscription,
    remove_user_push_subscription, save_push_subscription, take_new_countries, unmark_push_sent,
};
pub use unknown::{replace_unknown, report_no_cuisine, unknown, unknown_in, UnknownPlace};
pub use visits::{add_visit, remove_visit, visited_countries, visits, visits_between, Visit};

use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;

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

#[cfg(test)]
mod tests;
