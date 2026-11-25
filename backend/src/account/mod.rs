//! Accounts: anonymous users, one session per device, a recovery code that restores the
//! account anywhere, and short-lived codes that sign another device in.

use serde::Serialize;
use sqlx::{Row, SqlitePool};
use utoipa::ToSchema;

mod cleanup;
mod codes;
mod merge;
mod sessions;
mod sign_in;

pub use cleanup::{clean_up, run_cleanup, Cleaned};
pub(crate) use codes::random_code;
pub use codes::{new_link_code, new_recovery_code};
pub use sessions::{create_user, hash_plain_tokens, session_for_token};
pub use sign_in::{redeem, sign_in_with};

/// Crockford's base32: no I, L, O or U, so a code read aloud or off a screen is hard to
/// get wrong. 32 symbols, so a random byte modulo 32 is unbiased.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// 20 symbols, 100 bits: unguessable without any rate limit.
const RECOVERY_LEN: usize = 20;
/// 8 symbols, 40 bits, for at most [`LINK_TTL_SECS`] and one use.
const LINK_LEN: usize = 8;
pub const LINK_TTL_SECS: i64 = 600;
/// `last_used_at` is written at most this often, not on every request.
const TOUCH_SECS: i64 = 3600;
const MAX_DEVICE_NAME: usize = 60;
/// An account with nothing in it and no way back to it goes once it is this old and no
/// device used it for this long.
pub const ABANDONED_AFTER_SECS: i64 = 90 * 86_400;
/// How long it is remembered that a reminder was sent. Each is only due on one day.
const PUSH_SENT_KEPT_SECS: i64 = 60 * 86_400;
/// How often [`run_cleanup`] looks.
const CLEANUP_EVERY: std::time::Duration = std::time::Duration::from_hours(24);

/// The session a request was made with.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub user: String,
}

/// A device signed in to the account.
#[derive(Debug, Serialize, ToSchema)]
pub struct Device {
    /// Pass it to `DELETE /api/account/devices/{device_id}` to sign the device out.
    pub id: String,
    /// What the app called itself ("iPhone", "Android", "Web"), `null` for devices from
    /// before devices had names.
    #[schema(required = true)]
    pub name: Option<String>,
    pub created_at: i64,
    /// Updated about once an hour.
    pub last_used_at: i64,
    /// The device this request came from.
    pub current: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Account {
    pub created_at: i64,
    /// When the current recovery code was made, `null` when there is none yet.
    #[schema(required = true)]
    pub recovery_code_created_at: Option<i64>,
    /// Most recently used first.
    pub devices: Vec<Device>,
    /// The public page of the user's progress, like `/p/abcd2345ef`, made by sharing;
    /// `null` when it's off.
    #[schema(required = true)]
    pub public_profile: Option<String>,
    /// The ways to sign in to the account, oldest first; empty for an account that only
    /// lives on its devices.
    pub identities: Vec<Identity>,
}

/// A way to sign in: Sign in with Apple, Google or an email address.
#[derive(Debug, Serialize, ToSchema)]
pub struct Identity {
    /// `apple`, `google` or `email`.
    pub provider: String,
    /// The address it goes with, when known. Apple may give a private relay address.
    #[schema(required = true)]
    pub email: Option<String>,
    /// The person's name, from Google; `null` for Apple and email.
    #[schema(required = true)]
    pub name: Option<String>,
    /// A link to their profile photo, from Google; `null` for Apple and email.
    #[schema(required = true)]
    pub picture: Option<String>,
}

/// What an identity says about the person, as far as its provider shares it.
#[derive(Debug, Default)]
pub struct Profile<'a> {
    pub email: Option<&'a str>,
    pub name: Option<&'a str>,
    pub picture: Option<&'a str>,
}

pub async fn account(db: &SqlitePool, session: &Session) -> sqlx::Result<Account> {
    let row = sqlx::query("SELECT created_at, recovery_created_at FROM users WHERE id = ?")
        .bind(&session.user)
        .fetch_one(db)
        .await?;
    let devices = sqlx::query(
        "SELECT id, device, created_at, last_used_at FROM sessions WHERE user_id = ?
         ORDER BY last_used_at DESC, created_at DESC, rowid DESC",
    )
    .bind(&session.user)
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| {
        let id: String = r.get("id");
        Device {
            current: id == session.id,
            id,
            name: r.get("device"),
            created_at: r.get("created_at"),
            last_used_at: r.get("last_used_at"),
        }
    })
    .collect();
    Ok(Account {
        created_at: row.get("created_at"),
        recovery_code_created_at: row.get("recovery_created_at"),
        devices,
        public_profile: crate::share::profile_slug(db, &session.user).await?.map(|slug| format!("/p/{slug}")),
        identities: sqlx::query("SELECT provider, email, name, picture FROM identities WHERE user_id = ? ORDER BY created_at, provider")
            .bind(&session.user)
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|r| Identity { provider: r.get("provider"), email: r.get("email"), name: r.get("name"), picture: r.get("picture") })
            .collect(),
    })
}

/// Signs one of the user's devices out. Returns false when the user has no such device.
pub async fn remove_device(db: &SqlitePool, user: &str, device: &str) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM sessions WHERE id = ? AND user_id = ?")
        .bind(device)
        .bind(user)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Deletes the user and everything of theirs: sessions, visits, reports, added places,
/// push subscriptions (all cascade from `users`).
pub async fn delete(db: &SqlitePool, user: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user)
        .execute(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
