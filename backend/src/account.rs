//! Accounts: anonymous users, one session per device, a recovery code that restores the
//! account anywhere, and short-lived codes that sign another device in.

use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::fmt::Write as _;
use utoipa::ToSchema;

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

pub(crate) fn random_code(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    getrandom::fill(&mut bytes).expect("the system has no randomness");
    bytes
        .iter()
        .map(|b| ALPHABET[(*b % 32) as usize] as char)
        .collect()
}

/// Groups of four, as shown to people: `ABCD-EFGH`.
fn grouped(code: &str) -> String {
    code.as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join("-")
}

/// What people type back: any case, with or without dashes or spaces, O for 0 and I or L
/// for 1.
fn normalized(input: &str) -> String {
    input
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        })
        .collect()
}

fn hash(code: &str) -> String {
    Sha256::digest(code.as_bytes())
        .iter()
        .fold(String::new(), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        })
}

fn device_name(name: Option<&str>) -> Option<String> {
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
async fn start_session(
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
async fn insert_user(db: impl sqlx::SqliteExecutor<'_>, now: i64) -> sqlx::Result<String> {
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

/// Makes a new recovery code for `user`, replacing the old one, and returns it grouped.
/// Only its hash is kept, so it can't be shown again.
pub async fn new_recovery_code(db: &SqlitePool, user: &str, now: i64) -> sqlx::Result<String> {
    let code = random_code(RECOVERY_LEN);
    sqlx::query("UPDATE users SET recovery_hash = ?, recovery_created_at = ? WHERE id = ?")
        .bind(hash(&code))
        .bind(now)
        .bind(user)
        .execute(db)
        .await?;
    Ok(grouped(&code))
}

/// Makes a single-use code that signs another device in to `user`'s account for
/// [`LINK_TTL_SECS`]. Returns it grouped, with when it expires.
pub async fn new_link_code(db: &SqlitePool, user: &str, now: i64) -> sqlx::Result<(String, i64)> {
    let code = random_code(LINK_LEN);
    let expires_at = now + LINK_TTL_SECS;
    sqlx::query("DELETE FROM link_codes WHERE expires_at <= ?")
        .bind(now)
        .execute(db)
        .await?;
    sqlx::query("INSERT INTO link_codes (hash, user_id, expires_at) VALUES (?, ?, ?)")
        .bind(hash(&code))
        .bind(user)
        .bind(expires_at)
        .execute(db)
        .await?;
    Ok((grouped(&code), expires_at))
}

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

/// Every table and column that refers to `users`, read from the schema, so that a new
/// table of a user's is found without being listed anywhere.
async fn owned(db: impl sqlx::SqliteExecutor<'_>) -> sqlx::Result<Vec<(String, String)>> {
    sqlx::query_as(
        r#"SELECT m.name, f."from" FROM sqlite_master m, pragma_foreign_key_list(m.name) f
           WHERE m.type = 'table' AND f."table" = 'users' ORDER BY m.name"#,
    )
    .fetch_all(db)
    .await
}

/// Tables whose rows stay behind in a merge, and so go with the merged user: a link code
/// made for that account no longer signs anyone in.
const NOT_MERGED: &[&str] = &["link_codes"];

/// Moves everything of user `from` to user `into`, then deletes `from`. Where both have the
/// same row (a visit on the same day, a closed report of the same place, a public page)
/// `into`'s stays.
///
/// What a user has is read from the schema: every column that refers to `users`. A new
/// table of theirs is merged without being listed here.
async fn merge(tx: &mut sqlx::SqliteConnection, from: &str, into: &str) -> sqlx::Result<()> {
    for (table, column) in owned(&mut *tx).await? {
        if NOT_MERGED.contains(&table.as_str()) {
            continue;
        }
        let statement =
            format!(r#"UPDATE OR IGNORE "{table}" SET "{column}" = ?1 WHERE "{column}" = ?2"#);
        sqlx::query(&statement)
            .bind(into)
            .bind(from)
            .execute(&mut *tx)
            .await?;
    }
    // Rows left behind were duplicates; they go with the user.
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(from)
        .execute(&mut *tx)
        .await?;
    Ok(())
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

/// Tables whose rows don't make an account worth keeping: its sessions count by when they
/// were last used, and a link code is gone within [`LINK_TTL_SECS`].
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_grouped_and_forgiving_to_type() {
        let code = random_code(RECOVERY_LEN);
        assert_eq!(code.len(), RECOVERY_LEN);
        assert!(code.bytes().all(|b| ALPHABET.contains(&b)));
        let shown = grouped(&code);
        assert_eq!(shown.len(), RECOVERY_LEN + 4);
        assert_eq!(normalized(&shown), code);
        assert_eq!(normalized(&shown.to_lowercase().replace('-', " ")), code);
        assert_eq!(normalized("abcd-oil0"), "ABCD0110");
    }

    #[test]
    fn device_names_are_trimmed_and_capped() {
        assert_eq!(device_name(Some("  iPhone ")), Some("iPhone".into()));
        assert_eq!(device_name(Some("   ")), None);
        assert_eq!(
            device_name(Some(&"x".repeat(100))).unwrap().len(),
            MAX_DEVICE_NAME
        );
    }

    /// A database as it was before tokens were hashed, with one device signed in.
    #[tokio::test]
    async fn a_token_from_before_hashing_still_signs_in() {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let mut before = sqlx::migrate!();
        before.migrations = before
            .migrations
            .iter()
            .filter(|m| m.version < 18)
            .cloned()
            .collect();
        before.run(&db).await.unwrap();
        sqlx::query("INSERT INTO users (id, token, created_at) VALUES ('u', 'u', 0)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions (id, token, user_id, created_at, last_used_at) VALUES ('s', 'old-token', 'u', 0, 0)")
            .execute(&db)
            .await
            .unwrap();

        // Twice: the server does this at every start.
        crate::store::migrate(&db).await.unwrap();
        crate::store::migrate(&db).await.unwrap();

        let stored: Vec<String> = sqlx::query_scalar("SELECT token_hash FROM sessions")
            .fetch_all(&db)
            .await
            .unwrap();
        assert_eq!(stored, [hash("old-token")]);
        let session = session_for_token(&db, "old-token", 0).await.unwrap();
        assert_eq!(session.unwrap().user, "u");
        // What is stored signs nobody in.
        assert!(session_for_token(&db, &stored[0], 0)
            .await
            .unwrap()
            .is_none());
        assert!(session_for_token(&db, "plain:old-token", 0)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn a_new_token_is_stored_as_its_hash() {
        let db = crate::store::connect("sqlite::memory:").await.unwrap();
        let token = create_user(&db, None, 0).await.unwrap();
        let stored: String = sqlx::query_scalar("SELECT token_hash FROM sessions")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(stored, hash(&token));
        assert_ne!(stored, token);
        assert_eq!(hash_plain_tokens(&db).await.unwrap(), 0);
        assert!(session_for_token(&db, &token, 0).await.unwrap().is_some());
    }

    async fn users(db: &SqlitePool) -> Vec<String> {
        sqlx::query_scalar("SELECT id FROM users ORDER BY id")
            .fetch_all(db)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn an_empty_account_goes_once_it_is_old_and_unused() {
        const DAY: i64 = 86_400;
        let db = crate::store::connect("sqlite::memory:").await.unwrap();
        let token = create_user(&db, None, 0).await.unwrap();
        let user = session_for_token(&db, &token, 0)
            .await
            .unwrap()
            .unwrap()
            .user;

        // Too young, whatever it holds.
        let at = ABANDONED_AFTER_SECS;
        assert_eq!(clean_up(&db, at).await.unwrap().users, 0);
        // Old, but a device used it lately.
        session_for_token(&db, &token, at + 30 * DAY).await.unwrap();
        assert_eq!(clean_up(&db, at + 31 * DAY).await.unwrap().users, 0);
        assert_eq!(
            clean_up(&db, at + 30 * DAY + ABANDONED_AFTER_SECS)
                .await
                .unwrap()
                .users,
            0
        );
        assert_eq!(users(&db).await, std::slice::from_ref(&user));
        // A recovery code is a way back in.
        new_recovery_code(&db, &user, 0).await.unwrap();
        let later = at + 31 * DAY + ABANDONED_AFTER_SECS;
        assert_eq!(clean_up(&db, later).await.unwrap().users, 0);
        sqlx::query("UPDATE users SET recovery_hash = NULL")
            .execute(&db)
            .await
            .unwrap();
        // An unused link code is not.
        new_link_code(&db, &user, later).await.unwrap();

        assert_eq!(clean_up(&db, later).await.unwrap().users, 1);
        assert_eq!(users(&db).await, [] as [String; 0]);
        assert!(session_for_token(&db, &token, later)
            .await
            .unwrap()
            .is_none());
        // Without any session left it is just as gone.
        let token = create_user(&db, None, 0).await.unwrap();
        sqlx::query("DELETE FROM sessions")
            .execute(&db)
            .await
            .unwrap();
        assert!(session_for_token(&db, &token, 0).await.unwrap().is_none());
        assert_eq!(clean_up(&db, later).await.unwrap().users, 1);
    }

    /// One old, unused user per table with a column that names a user, each with a row in
    /// just that table. Found by the column's name and not the way `clean_up` finds them.
    #[tokio::test]
    async fn an_account_with_a_row_in_any_table_is_kept() {
        let db = crate::store::connect("sqlite::memory:").await.unwrap();
        let mut conn = db.acquire().await.unwrap();
        // The rows are made up, as in `a_merge_leaves_no_row_of_the_user_behind`.
        for pragma in ["foreign_keys = OFF", "ignore_check_constraints = ON"] {
            sqlx::query(&format!("PRAGMA {pragma}"))
                .execute(&mut *conn)
                .await
                .unwrap();
        }
        let named: Vec<(String, String)> = sqlx::query_as(
            "SELECT m.name, c.name FROM sqlite_master m, pragma_table_info(m.name) c
             WHERE m.type = 'table' AND c.name IN ('user_id', 'added_by') ORDER BY m.name",
        )
        .fetch_all(&mut *conn)
        .await
        .unwrap();
        assert!(named.len() >= 10, "{named:?}");
        for (table, user_column) in &named {
            sqlx::query("INSERT INTO users (id, token, created_at) VALUES (?, ?, 0)")
                .bind(table)
                .bind(table)
                .execute(&mut *conn)
                .await
                .unwrap();
            let columns: Vec<(String, String)> =
                sqlx::query_as("SELECT name, type FROM pragma_table_info(?)")
                    .bind(table)
                    .fetch_all(&mut *conn)
                    .await
                    .unwrap();
            let quoted: Vec<String> = columns.iter().map(|(n, _)| format!(r#""{n}""#)).collect();
            let values: Vec<String> = columns
                .iter()
                .map(
                    |(column, kind)| match (column == user_column, kind.as_str()) {
                        (true, _) => format!("'{table}'"),
                        // Unique per row: some of these columns are keys.
                        (false, "TEXT") => format!("'x-{table}'"),
                        // Also the time a session was last used: long ago.
                        (false, _) => "1".into(),
                    },
                )
                .collect();
            let insert = format!(
                r#"INSERT INTO "{table}" ({}) VALUES ({})"#,
                quoted.join(", "),
                values.join(", ")
            );
            sqlx::query(&insert).execute(&mut *conn).await.unwrap();
        }
        drop(conn);

        let cleaned = clean_up(&db, 10 * ABANDONED_AFTER_SECS).await.unwrap();

        // Named here again on purpose: what doesn't keep an account is a decision.
        let kept: Vec<String> = named
            .iter()
            .map(|(table, _)| table.clone())
            .filter(|table| table != "sessions" && table != "link_codes")
            .collect();
        assert_eq!(users(&db).await, kept);
        assert_eq!(cleaned.users, 2);
    }

    #[tokio::test]
    async fn expired_codes_and_old_reminder_records_are_deleted() {
        let db = crate::store::connect("sqlite::memory:").await.unwrap();
        let token = create_user(&db, None, 0).await.unwrap();
        let user = session_for_token(&db, &token, 0)
            .await
            .unwrap()
            .unwrap()
            .user;
        let now = 100 * 86_400;
        for (email, expires_at) in [("old@example.com", now), ("new@example.com", now + 1)] {
            sqlx::query("INSERT INTO email_codes (email, hash, expires_at) VALUES (?, 'h', ?)")
                .bind(email)
                .bind(expires_at)
                .execute(&db)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO push_subscriptions (endpoint, user_id, p256dh, auth, timezone, created_at) VALUES ('e', ?, 'p', 'a', 'UTC', 0)")
            .bind(&user)
            .execute(&db)
            .await
            .unwrap();
        for (key, sent_at) in [
            ("old", now - PUSH_SENT_KEPT_SECS - 1),
            ("new", now - PUSH_SENT_KEPT_SECS),
        ] {
            sqlx::query("INSERT INTO push_sent (endpoint, key, sent_at) VALUES ('e', ?, ?)")
                .bind(key)
                .bind(sent_at)
                .execute(&db)
                .await
                .unwrap();
        }
        // In this order: making a link code deletes the ones that expired before it.
        new_link_code(&db, &user, now).await.unwrap();
        new_link_code(&db, &user, now - LINK_TTL_SECS)
            .await
            .unwrap();

        let cleaned = clean_up(&db, now).await.unwrap();

        assert_eq!(
            cleaned,
            Cleaned {
                users: 0,
                email_codes: 1,
                link_codes: 1,
                push_sent: 1
            }
        );
        let emails: Vec<String> = sqlx::query_scalar("SELECT email FROM email_codes")
            .fetch_all(&db)
            .await
            .unwrap();
        assert_eq!(emails, ["new@example.com"]);
        let keys: Vec<String> = sqlx::query_scalar("SELECT key FROM push_sent")
            .fetch_all(&db)
            .await
            .unwrap();
        assert_eq!(keys, ["new"]);
        assert_eq!(clean_up(&db, now).await.unwrap(), Cleaned::default());
    }

    /// Every table that refers to a user, with a row of the merged user's in it.
    #[tokio::test]
    async fn a_merge_moves_every_table_of_the_user() {
        let db = crate::store::connect("sqlite::memory:").await.unwrap();
        let from = create_user(&db, None, 0).await.unwrap();
        let from = session_for_token(&db, &from, 0)
            .await
            .unwrap()
            .unwrap()
            .user;
        let into = create_user(&db, None, 0).await.unwrap();
        let into = session_for_token(&db, &into, 0)
            .await
            .unwrap()
            .unwrap()
            .user;
        sqlx::query("INSERT INTO no_cuisine_reports (place_id, user_id, reported_at) VALUES ('osm-node-1', ?, 0)")
            .bind(&from)
            .execute(&db)
            .await
            .unwrap();
        new_link_code(&db, &from, 0).await.unwrap();

        let mut tx = db.begin().await.unwrap();
        merge(&mut tx, &from, &into).await.unwrap();
        tx.commit().await.unwrap();

        let reports: Vec<String> = sqlx::query_scalar("SELECT user_id FROM no_cuisine_reports")
            .fetch_all(&db)
            .await
            .unwrap();
        assert_eq!(reports, std::slice::from_ref(&into));
        let sessions: Vec<String> = sqlx::query_scalar("SELECT DISTINCT user_id FROM sessions")
            .fetch_all(&db)
            .await
            .unwrap();
        assert_eq!(sessions, [into]);
        let codes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM link_codes")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(codes, 0);
    }

    /// A row in every table with a column that names a user, found here by the column's
    /// name and not the way `merge` finds them: after a merge each row is the other user's,
    /// or its table is one that says it stays behind.
    #[tokio::test]
    async fn a_merge_leaves_no_row_of_the_user_behind() {
        let db = crate::store::connect("sqlite::memory:").await.unwrap();
        let mut conn = db.acquire().await.unwrap();
        // The rows are made up: what they refer to isn't there, and a value may be one a
        // column doesn't allow.
        for pragma in ["foreign_keys = OFF", "ignore_check_constraints = ON"] {
            sqlx::query(&format!("PRAGMA {pragma}"))
                .execute(&mut *conn)
                .await
                .unwrap();
        }
        let named: Vec<(String, String)> = sqlx::query_as(
            "SELECT m.name, c.name FROM sqlite_master m, pragma_table_info(m.name) c
             WHERE m.type = 'table' AND c.name IN ('user_id', 'added_by') ORDER BY m.name",
        )
        .fetch_all(&mut *conn)
        .await
        .unwrap();
        assert!(named.len() >= 10, "{named:?}");
        for id in ["from", "into"] {
            sqlx::query("INSERT INTO users (id, token, created_at) VALUES (?, ?, 0)")
                .bind(id)
                .bind(id)
                .execute(&mut *conn)
                .await
                .unwrap();
        }
        for (table, user_column) in &named {
            let columns: Vec<(String, String)> =
                sqlx::query_as("SELECT name, type FROM pragma_table_info(?)")
                    .bind(table)
                    .fetch_all(&mut *conn)
                    .await
                    .unwrap();
            let quoted: Vec<String> = columns.iter().map(|(n, _)| format!(r#""{n}""#)).collect();
            let values: Vec<&str> = columns
                .iter()
                .map(
                    |(column, kind)| match (column == user_column, kind.as_str()) {
                        (true, _) => "'from'",
                        (false, "TEXT") => "'x'",
                        (false, _) => "1",
                    },
                )
                .collect();
            let insert = format!(
                r#"INSERT INTO "{table}" ({}) VALUES ({})"#,
                quoted.join(", "),
                values.join(", ")
            );
            sqlx::query(&insert).execute(&mut *conn).await.unwrap();
        }

        merge(&mut conn, "from", "into").await.unwrap();

        for (table, column) in &named {
            let owners: Vec<String> =
                sqlx::query_scalar(&format!(r#"SELECT "{column}" FROM "{table}""#))
                    .fetch_all(&mut *conn)
                    .await
                    .unwrap();
            // Named here again on purpose: leaving a table out of a merge is a decision.
            let expected = if table == "link_codes" {
                "from"
            } else {
                "into"
            };
            assert_eq!(owners, [expected], "{table}.{column}");
        }
    }
}
