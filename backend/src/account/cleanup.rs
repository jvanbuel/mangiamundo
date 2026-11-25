//! Deleting abandoned accounts and expired codes.

use std::fmt::Write as _;

use sqlx::SqlitePool;

use super::merge::owned;
use super::{ABANDONED_AFTER_SECS, CLEANUP_EVERY, PUSH_SENT_KEPT_SECS};

/// Tables whose rows don't make an account worth keeping: its sessions count by when they
/// were last used, and a link code is gone within [`LINK_TTL_SECS`].
///
/// [`LINK_TTL_SECS`]: super::LINK_TTL_SECS
const NOT_DATA: &[&str] = &["sessions", "link_codes"];

/// What a [`clean_up`] deleted.
#[derive(Debug, Default, PartialEq)]
pub struct Cleaned {
    pub users: u64,
    pub email_codes: u64,
    pub link_codes: u64,
    pub push_sent: u64,
}

/// Deletes what nobody will come back for: abandoned accounts, expired sign-in and link
/// codes, and the record of reminders sent long ago.
///
/// An account is abandoned when all of this holds: it is older than
/// [`ABANDONED_AFTER_SECS`], none of its devices used it since, it has no recovery code and
/// no identity to sign in with, and no row of any table refers to it apart from its
/// sessions and link codes. Like [`merge`], the tables are read from the schema: a row in
/// a table added later keeps its account.
///
/// [`merge`]: super::merge::merge
pub async fn clean_up(db: &SqlitePool, now: i64) -> sqlx::Result<Cleaned> {
    let mut tx = db.begin().await?;
    let mut abandoned = String::from(
        "DELETE FROM users WHERE created_at < ?1 AND recovery_hash IS NULL
         AND NOT EXISTS (SELECT 1 FROM sessions WHERE user_id = users.id AND last_used_at >= ?1)",
    );
    for (table, column) in owned(&mut *tx).await? {
        if !NOT_DATA.contains(&table.as_str()) {
            let _ = write!(
                abandoned,
                r#" AND NOT EXISTS (SELECT 1 FROM "{table}" WHERE "{column}" = users.id)"#
            );
        }
    }
    let users = sqlx::query(&abandoned)
        .bind(now - ABANDONED_AFTER_SECS)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    let email_codes = sqlx::query("DELETE FROM email_codes WHERE expires_at <= ?")
        .bind(now)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    let link_codes = sqlx::query("DELETE FROM link_codes WHERE expires_at <= ?")
        .bind(now)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    let push_sent = sqlx::query("DELETE FROM push_sent WHERE sent_at < ?")
        .bind(now - PUSH_SENT_KEPT_SECS)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    tx.commit().await?;
    Ok(Cleaned {
        users,
        email_codes,
        link_codes,
        push_sent,
    })
}

/// Cleans up once a day, starting now.
pub async fn run_cleanup(state: crate::AppState) {
    let mut ticker = tokio::time::interval(CLEANUP_EVERY);
    loop {
        ticker.tick().await;
        match clean_up(&state.db, state.now()).await {
            Ok(cleaned) if cleaned == Cleaned::default() => {}
            Ok(cleaned) => tracing::info!(?cleaned, "cleaned up"),
            Err(error) => tracing::error!(%error, "cleanup failed"),
        }
    }
}
