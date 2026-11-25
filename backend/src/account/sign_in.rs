//! Signing a device in to an account with a code or an identity.

use sqlx::SqlitePool;

use super::codes::{hash, normalized};
use super::merge::merge;
use super::sessions::{insert_user, start_session};
use super::{Profile, Session, LINK_LEN, RECOVERY_LEN};

/// Signs this device in to the account a recovery or link code belongs to, and returns a
/// token for it, or `None` for an unknown, used or expired code.
///
/// When the device was already signed in to another account (`current`), that account's
/// visits and everything else move into the restored one and it is deleted, so nothing
/// made on the device before restoring is lost. The old session ends either way.
pub async fn redeem(
    db: &SqlitePool,
    code: &str,
    current: Option<&Session>,
    device: Option<&str>,
    now: i64,
) -> sqlx::Result<Option<String>> {
    let code = normalized(code);
    let mut tx = db.begin().await?;
    let user: Option<String> = match code.len() {
        RECOVERY_LEN => {
            sqlx::query_scalar("SELECT id FROM users WHERE recovery_hash = ?")
                .bind(hash(&code))
                .fetch_optional(&mut *tx)
                .await?
        }
        LINK_LEN => {
            sqlx::query_scalar(
                "DELETE FROM link_codes WHERE hash = ? AND expires_at > ? RETURNING user_id",
            )
            .bind(hash(&code))
            .bind(now)
            .fetch_optional(&mut *tx)
            .await?
        }
        _ => None,
    };
    let Some(user) = user else { return Ok(None) };
    let token = switch_to(&mut tx, &user, current, device, now).await?;
    tx.commit().await?;
    Ok(Some(token))
}

/// Signs this device in as `user`: the `current` session ends, and when it belonged to
/// another account, that account merges into `user`. Returns the new session's token.
async fn switch_to(
    tx: &mut sqlx::SqliteConnection,
    user: &str,
    current: Option<&Session>,
    device: Option<&str>,
    now: i64,
) -> sqlx::Result<String> {
    if let Some(old) = current {
        // The owner is read again here: a concurrent restore may have moved this session to
        // another account since `old` was looked up. A session that is gone (signed out, its
        // account deleted) has nothing left to merge.
        let owner: Option<String> =
            sqlx::query_scalar("DELETE FROM sessions WHERE id = ? RETURNING user_id")
                .bind(&old.id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(owner) = owner.filter(|owner| owner != user) {
            merge(tx, &owner, user).await?;
        }
    }
    start_session(&mut *tx, user, device, now).await
}

/// Signs in with an identity (Apple, Google or an email address). When an account already
/// has it, this device switches to that account, bringing along what the current one holds.
/// Otherwise the identity joins the current account, or a new one without a session.
pub async fn sign_in_with(
    db: &SqlitePool,
    provider: &str,
    subject: &str,
    profile: &Profile<'_>,
    current: Option<&Session>,
    device: Option<&str>,
    now: i64,
) -> sqlx::Result<String> {
    let mut tx = db.begin().await?;
    let owner: Option<String> =
        sqlx::query_scalar("SELECT user_id FROM identities WHERE provider = ? AND subject = ?")
            .bind(provider)
            .bind(subject)
            .fetch_optional(&mut *tx)
            .await?;
    let user = if let Some(user) = owner {
        // Apple sends the address only the first time; keep what we have when a value is
        // missing, and take Google's latest name and photo.
        sqlx::query(
            "UPDATE identities SET email = COALESCE(?, email), name = COALESCE(?, name),
             picture = COALESCE(?, picture) WHERE provider = ? AND subject = ?",
        )
        .bind(profile.email)
        .bind(profile.name)
        .bind(profile.picture)
        .bind(provider)
        .bind(subject)
        .execute(&mut *tx)
        .await?;
        user
    } else {
        let existing: Option<String> = match current {
            Some(session) => {
                sqlx::query_scalar("SELECT user_id FROM sessions WHERE id = ?")
                    .bind(&session.id)
                    .fetch_optional(&mut *tx)
                    .await?
            }
            None => None,
        };
        let user = match existing {
            Some(user) => user,
            None => insert_user(&mut *tx, now).await?,
        };
        sqlx::query("INSERT INTO identities (provider, subject, user_id, email, name, picture, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(provider)
            .bind(subject)
            .bind(&user)
            .bind(profile.email)
            .bind(profile.name)
            .bind(profile.picture)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        user
    };
    let token = switch_to(&mut tx, &user, current, device, now).await?;
    tx.commit().await?;
    Ok(token)
}
