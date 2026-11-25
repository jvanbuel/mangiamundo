//! Signing in with a one-time code sent by email.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};

use super::{Auth, CODE_ATTEMPTS, CODE_TTL_SECS};

impl Auth {
    /// Sends a sign-in code. Without a Resend key it goes to the log.
    pub async fn send_code(&self, email: &str, code: &str) -> anyhow::Result<()> {
        let Some(key) = &self.config.resend_key else {
            tracing::warn!(%email, %code, "no RESEND_API_KEY: sign-in code not sent, only logged");
            return Ok(());
        };
        let text = format!(
            "Your Mangiamundo sign-in code is {code}.\n\nIt works for 10 minutes. If you didn't ask for it, you can ignore this email."
        );
        self.http
            .post(&self.config.resend_url)
            .bearer_auth(key)
            .json(&serde_json::json!({
                "from": self.config.mail_from,
                "to": [email],
                "subject": format!("{code} is your Mangiamundo code"),
                "text": text,
            }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

/// "  Jan@Example.COM " → "jan@example.com"; `None` for something that isn't an address.
pub fn normalized_email(input: &str) -> Option<String> {
    let email = input.trim().to_lowercase();
    let (local, domain) = email.split_once('@')?;
    let ok = !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && email.len() <= 254
        && !email
            .chars()
            .any(|c| c.is_whitespace() || c == ',' || c == '<' || c == '>');
    ok.then_some(email)
}

fn code_hash(email: &str, code: &str) -> String {
    let digest = Sha256::digest(format!("{email}:{code}").as_bytes());
    digest.iter().fold(String::new(), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    })
}

/// Makes a new 6-digit code for the address, replacing any earlier one, and returns it.
pub async fn new_code(db: &SqlitePool, email: &str, now: i64) -> sqlx::Result<String> {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).expect("the system has no randomness");
    let code = format!("{:06}", u32::from_le_bytes(bytes) % 1_000_000);
    sqlx::query(
        "INSERT INTO email_codes (email, hash, expires_at, attempts) VALUES (?, ?, ?, 0)
         ON CONFLICT (email) DO UPDATE SET hash = excluded.hash, expires_at = excluded.expires_at, attempts = 0",
    )
    .bind(email)
    .bind(code_hash(email, &code))
    .bind(now + CODE_TTL_SECS)
    .execute(db)
    .await?;
    Ok(code)
}

/// Uses up the address's code when `code` matches. A wrong one counts as an attempt.
pub async fn take_code(db: &SqlitePool, email: &str, code: &str, now: i64) -> sqlx::Result<bool> {
    let code: String = code.chars().filter(char::is_ascii_digit).collect();
    let Some(row) =
        sqlx::query("SELECT hash, expires_at, attempts FROM email_codes WHERE email = ?")
            .bind(email)
            .fetch_optional(db)
            .await?
    else {
        return Ok(false);
    };
    if row.get::<i64, _>("expires_at") <= now || row.get::<i64, _>("attempts") >= CODE_ATTEMPTS {
        return Ok(false);
    }
    if row.get::<String, _>("hash") != code_hash(email, &code) {
        sqlx::query("UPDATE email_codes SET attempts = attempts + 1 WHERE email = ?")
            .bind(email)
            .execute(db)
            .await?;
        return Ok(false);
    }
    sqlx::query("DELETE FROM email_codes WHERE email = ?")
        .bind(email)
        .execute(db)
        .await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_are_normalized_and_checked() {
        assert_eq!(
            normalized_email("  Jan@Example.COM "),
            Some("jan@example.com".into())
        );
        assert_eq!(normalized_email("no-at-sign"), None);
        assert_eq!(normalized_email("a@localhost"), None);
        assert_eq!(normalized_email("a@b.c, x@y.z"), None);
    }
}
