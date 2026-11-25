//! Web Push subscriptions and what was sent to them.

use sqlx::SqlitePool;

/// Adds a browser subscription, or moves an existing one to a new user, area or time zone.
pub async fn save_push_subscription(
    db: &SqlitePool,
    user: &str,
    sub: &crate::push::Subscription,
    at: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO push_subscriptions (endpoint, user_id, p256dh, auth, area_id, timezone, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(endpoint) DO UPDATE SET user_id = excluded.user_id, p256dh = excluded.p256dh,
             auth = excluded.auth, area_id = excluded.area_id, timezone = excluded.timezone,
             -- Another area starts over from what is there, even one known before.
             known_area_id = CASE WHEN area_id IS excluded.area_id THEN known_area_id END",
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
    sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = ?")
        .bind(endpoint)
        .execute(db)
        .await?;
    Ok(())
}

/// Removes a subscription only when it belongs to `user`. Returns whether one was removed.
pub async fn remove_user_push_subscription(
    db: &SqlitePool,
    user: &str,
    endpoint: &str,
) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = ? AND user_id = ?")
        .bind(endpoint)
        .bind(user)
        .execute(db)
        .await?;
    Ok(done.rows_affected() > 0)
}

pub async fn push_subscriptions(
    db: &SqlitePool,
) -> sqlx::Result<Vec<(String, crate::push::Subscription)>> {
    let rows = sqlx::query("SELECT * FROM push_subscriptions")
        .fetch_all(db)
        .await?;
    Ok(rows
        .iter()
        .map(crate::push::subscription_from_row)
        .collect())
}

/// Records that a reminder is being sent. False when it was sent before.
pub async fn mark_push_sent(
    db: &SqlitePool,
    endpoint: &str,
    key: &str,
    at: i64,
) -> sqlx::Result<bool> {
    let done =
        sqlx::query("INSERT OR IGNORE INTO push_sent (endpoint, key, sent_at) VALUES (?, ?, ?)")
            .bind(endpoint)
            .bind(key)
            .bind(at)
            .execute(db)
            .await?;
    Ok(done.rows_affected() > 0)
}

/// Marks the countries in `present` as told, returning those that were not yet.
///
/// The first look at an area, after subscribing or moving, marks them all and returns none.
pub async fn take_new_countries(
    db: &SqlitePool,
    endpoint: &str,
    area_id: &str,
    present: &[String],
) -> sqlx::Result<Vec<String>> {
    let mut tx = db.begin().await?;
    let moved = sqlx::query("UPDATE push_subscriptions SET known_area_id = ?2 WHERE endpoint = ?1 AND known_area_id IS NOT ?2")
        .bind(endpoint)
        .bind(area_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
        > 0;
    if moved {
        sqlx::query("DELETE FROM push_known_countries WHERE endpoint = ?")
            .bind(endpoint)
            .execute(&mut *tx)
            .await?;
    }
    let mut new = Vec::new();
    for country in present {
        let done = sqlx::query(
            "INSERT OR IGNORE INTO push_known_countries (endpoint, country) VALUES (?, ?)",
        )
        .bind(endpoint)
        .bind(country)
        .execute(&mut *tx)
        .await?;
        if done.rows_affected() > 0 && !moved {
            new.push(country.clone());
        }
    }
    tx.commit().await?;
    Ok(new)
}

/// Undoes `take_new_countries` when telling the subscriber failed.
pub async fn forget_countries(
    db: &SqlitePool,
    endpoint: &str,
    countries: &[String],
) -> sqlx::Result<()> {
    for country in countries {
        sqlx::query("DELETE FROM push_known_countries WHERE endpoint = ? AND country = ?")
            .bind(endpoint)
            .bind(country)
            .execute(db)
            .await?;
    }
    Ok(())
}

pub async fn unmark_push_sent(db: &SqlitePool, endpoint: &str, key: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM push_sent WHERE endpoint = ? AND key = ?")
        .bind(endpoint)
        .bind(key)
        .execute(db)
        .await?;
    Ok(())
}
