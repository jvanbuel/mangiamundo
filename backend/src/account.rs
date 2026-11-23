//! Accounts: anonymous users, one session per device, a recovery code that restores the
//! account anywhere, and short-lived codes that sign another device in.

use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
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
        .filter(|c| c.is_ascii_alphanumeric())
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
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn device_name(name: Option<&str>) -> Option<String> {
    let name = name?.trim();
    (!name.is_empty()).then(|| name.chars().take(MAX_DEVICE_NAME).collect())
}

/// Starts a session for `user` and returns its token.
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
    sqlx::query("INSERT INTO sessions (id, token, user_id, device, created_at, last_used_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(uuid::Uuid::new_v4().simple().to_string())
        .bind(&token)
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
    let Some(row) = sqlx::query("SELECT id, user_id, last_used_at FROM sessions WHERE token = ?")
        .bind(token)
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
        identities: sqlx::query("SELECT provider, email FROM identities WHERE user_id = ? ORDER BY created_at, provider")
            .bind(&session.user)
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|r| Identity { provider: r.get("provider"), email: r.get("email") })
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
    email: Option<&str>,
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
    let user = match owner {
        Some(user) => {
            // Apple sends the address only the first time; keep the one we have then.
            if email.is_some() {
                sqlx::query("UPDATE identities SET email = ? WHERE provider = ? AND subject = ?")
                    .bind(email)
                    .bind(provider)
                    .bind(subject)
                    .execute(&mut *tx)
                    .await?;
            }
            user
        }
        None => {
            let existing: Option<String> = match current {
                Some(session) => sqlx::query_scalar("SELECT user_id FROM sessions WHERE id = ?")
                    .bind(&session.id)
                    .fetch_optional(&mut *tx)
                    .await?,
                None => None,
            };
            let user = match existing {
                Some(user) => user,
                None => insert_user(&mut *tx, now).await?,
            };
            sqlx::query("INSERT INTO identities (provider, subject, user_id, email, created_at) VALUES (?, ?, ?, ?, ?)")
                .bind(provider)
                .bind(subject)
                .bind(&user)
                .bind(email)
                .bind(now)
                .execute(&mut *tx)
                .await?;
            user
        }
    };
    let token = switch_to(&mut tx, &user, current, device, now).await?;
    tx.commit().await?;
    Ok(token)
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
    let owned: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT m.name, f."from" FROM sqlite_master m, pragma_foreign_key_list(m.name) f
           WHERE m.type = 'table' AND f."table" = 'users' ORDER BY m.name"#,
    )
    .fetch_all(&mut *tx)
    .await?;
    for (table, column) in owned {
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

    /// Every table that refers to a user, with a row of the merged user's in it.
    #[tokio::test]
    async fn a_merge_moves_every_table_of_the_user() {
        let db = crate::store::connect("sqlite::memory:").await.unwrap();
        let from = create_user(&db, None, 0).await.unwrap();
        let from = session_for_token(&db, &from, 0).await.unwrap().unwrap().user;
        let into = create_user(&db, None, 0).await.unwrap();
        let into = session_for_token(&db, &into, 0).await.unwrap().unwrap().user;
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
        assert_eq!(reports, [into.clone()]);
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

    /// `merge` finds a user's rows through foreign keys to `users`. A column that names a
    /// user without one would be left behind, so there may be none.
    #[tokio::test]
    async fn every_column_naming_a_user_refers_to_users() {
        let db = crate::store::connect("sqlite::memory:").await.unwrap();
        let named: Vec<(String, String)> = sqlx::query_as(
            "SELECT m.name, c.name FROM sqlite_master m, pragma_table_info(m.name) c
             WHERE m.type = 'table' AND c.name IN ('user_id', 'added_by') ORDER BY m.name",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert!(named.len() >= 10, "{named:?}");
        for (table, column) in named {
            let refers: i64 = sqlx::query_scalar(
                r#"SELECT COUNT(*) FROM pragma_foreign_key_list(?) WHERE "table" = 'users' AND "from" = ?"#,
            )
            .bind(&table)
            .bind(&column)
            .fetch_one(&db)
            .await
            .unwrap();
            assert_eq!(refers, 1, "{table}.{column} has no foreign key to users");
        }
    }
}
