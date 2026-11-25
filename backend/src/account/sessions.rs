//! Users and their sessions, one per device.

use sqlx::{Row, SqlitePool};

use super::codes::hash;
use super::{Session, MAX_DEVICE_NAME, TOUCH_SECS};

pub(super) fn device_name(name: Option<&str>) -> Option<String> {
    let name = name?.trim();
    (!name.is_empty()).then(|| name.chars().take(MAX_DEVICE_NAME).collect())
}

/// What migration 0018 put before the session tokens it found in plain text.
const PLAIN: &str = "plain:";

/// Hashes the session tokens from before only their hash was kept. Run at every start,
/// after the migrations: it only touches the rows migration 0018 marked, so a second run
/// finds nothing. Returns how many it hashed.
pub async fn hash_plain_tokens(db: &SqlitePool) -> sqlx::Result<usize> {
    let mut tx = db.begin().await?;
    let marked: Vec<(String, String)> =
        sqlx::query_as("SELECT id, token_hash FROM sessions WHERE token_hash LIKE ? || '%'")
            .bind(PLAIN)
            .fetch_all(&mut *tx)
            .await?;
    let mut hashed = 0;
    for (id, value) in &marked {
        // LIKE ignores case; only the exact mark counts.
        let Some(token) = value.strip_prefix(PLAIN) else {
            continue;
        };
        sqlx::query("UPDATE sessions SET token_hash = ? WHERE id = ?")
            .bind(hash(token))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        hashed += 1;
    }
    tx.commit().await?;
    Ok(hashed)
}

/// Starts a session for `user` and returns its token. Only the token's hash is kept: the
/// token can't be read back from the database.
pub(super) async fn start_session(
    db: impl sqlx::SqliteExecutor<'_>,
    user: &str,
    device: Option<&str>,
    now: i64,
) -> sqlx::Result<String> {
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    sqlx::query("INSERT INTO sessions (id, token_hash, user_id, device, created_at, last_used_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(uuid::Uuid::new_v4().simple().to_string())
        .bind(hash(&token))
        .bind(user)
        .bind(device_name(device))
        .bind(now)
        .bind(now)
        .execute(db)
        .await?;
    Ok(token)
}

/// Adds a user and returns its id.
pub(super) async fn insert_user(
    db: impl sqlx::SqliteExecutor<'_>,
    now: i64,
) -> sqlx::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    // `token` is a leftover column that must stay unique; the id is (see migration 0011).
    sqlx::query("INSERT INTO users (id, token, created_at) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(&id)
        .bind(now)
        .execute(db)
        .await?;
    Ok(id)
}

/// A new anonymous user with one session. Returns its token.
pub async fn create_user(db: &SqlitePool, device: Option<&str>, now: i64) -> sqlx::Result<String> {
    let mut tx = db.begin().await?;
    let id = insert_user(&mut *tx, now).await?;
    let token = start_session(&mut *tx, &id, device, now).await?;
    tx.commit().await?;
    Ok(token)
}

/// The session behind a token, marking it used.
pub async fn session_for_token(
    db: &SqlitePool,
    token: &str,
    now: i64,
) -> sqlx::Result<Option<Session>> {
    let Some(row) =
        sqlx::query("SELECT id, user_id, last_used_at FROM sessions WHERE token_hash = ?")
            .bind(hash(token))
            .fetch_optional(db)
            .await?
    else {
        return Ok(None);
    };
    let id: String = row.get("id");
    if now - row.get::<i64, _>("last_used_at") >= TOUCH_SECS {
        sqlx::query("UPDATE sessions SET last_used_at = ? WHERE id = ?")
            .bind(now)
            .bind(&id)
            .execute(db)
            .await?;
    }
    Ok(Some(Session {
        id,
        user: row.get("user_id"),
    }))
}
