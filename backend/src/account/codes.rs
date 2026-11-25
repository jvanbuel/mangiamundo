//! Recovery and link codes: how they look, how people type them back, and how they are kept.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use super::{ALPHABET, LINK_LEN, LINK_TTL_SECS, RECOVERY_LEN};

pub(crate) fn random_code(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    getrandom::fill(&mut bytes).expect("the system has no randomness");
    bytes
        .iter()
        .map(|b| ALPHABET[(*b % 32) as usize] as char)
        .collect()
}

/// Groups of four, as shown to people: `ABCD-EFGH`.
pub(super) fn grouped(code: &str) -> String {
    code.as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join("-")
}

/// What people type back: any case, with or without dashes or spaces, O for 0 and I or L
/// for 1.
pub(super) fn normalized(input: &str) -> String {
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

pub(super) fn hash(code: &str) -> String {
    Sha256::digest(code.as_bytes())
        .iter()
        .fold(String::new(), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
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
