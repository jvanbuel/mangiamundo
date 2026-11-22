//! Signing in with Apple, Google or a one-time code sent by email. The apps get an ID token
//! from Apple or Google and send it here; its signature is checked against the provider's
//! published keys, so no third-party auth service is involved.

use std::collections::HashMap;
use std::sync::Mutex;

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};

/// How long an emailed code works.
pub const CODE_TTL_SECS: i64 = 600;
/// Wrong guesses before a code stops working: 5 in a million, so guessing doesn't pay.
const CODE_ATTEMPTS: i64 = 5;
/// Published keys are fetched again after this, or sooner for a key id we don't know.
const KEYS_TTL_SECS: i64 = 6 * 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Provider {
    Apple,
    Google,
}

impl Provider {
    pub fn name(self) -> &'static str {
        match self {
            Provider::Apple => "apple",
            Provider::Google => "google",
        }
    }

    fn issuers(self) -> &'static [&'static str] {
        match self {
            Provider::Apple => &["https://appleid.apple.com"],
            Provider::Google => &["https://accounts.google.com", "accounts.google.com"],
        }
    }
}

/// Sign-in settings.
pub struct AuthConfig {
    /// Accepted audiences of Apple ID tokens: the app's bundle id and the web's Services ID.
    pub apple_client_ids: Vec<String>,
    /// The Services ID the web app signs in with, when Sign in with Apple is set up there.
    pub apple_web_client_id: Option<String>,
    /// Accepted audiences of Google ID tokens: the web client id (Android uses it too) and
    /// the iOS one.
    pub google_client_ids: Vec<String>,
    /// The web client id, for Google's sign-in button.
    pub google_web_client_id: Option<String>,
    pub apple_keys_url: String,
    pub google_keys_url: String,
    /// Resend's send endpoint. Without a key, codes are logged instead, for development.
    pub resend_url: String,
    pub resend_key: Option<String>,
    /// "Mangiamundo <hello@mangiamundo.com>".
    pub mail_from: String,
}

/// The settings and the providers' keys, kept between requests.
#[derive(Default)]
pub struct Auth {
    pub config: AuthConfig,
    keys: Mutex<HashMap<Provider, (JwkSet, i64)>>,
    http: reqwest::Client,
}

impl Auth {
    pub fn new(config: AuthConfig) -> Self {
        Auth { config, ..Default::default() }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig {
            apple_client_ids: Vec::new(),
            apple_web_client_id: None,
            google_client_ids: Vec::new(),
            google_web_client_id: None,
            apple_keys_url: "https://appleid.apple.com/auth/keys".into(),
            google_keys_url: "https://www.googleapis.com/oauth2/v3/certs".into(),
            resend_url: "https://api.resend.com/emails".into(),
            resend_key: None,
            mail_from: "Mangiamundo <hello@mangiamundo.com>".into(),
        }
    }
}

/// Who an ID token says signed in.
#[derive(Debug, PartialEq)]
pub struct Verified {
    pub subject: String,
    /// Only when the provider vouches for it.
    pub email: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("{0} sign-in isn't set up on this server")]
    NotConfigured(&'static str),
    #[error("the sign-in token is invalid or expired")]
    Invalid,
    #[error("could not reach {0}")]
    Upstream(&'static str, #[source] anyhow::Error),
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    email: Option<String>,
    /// A bool from Google, a "true"/"false" string from Apple.
    email_verified: Option<serde_json::Value>,
}

impl Auth {
    fn audiences(&self, provider: Provider) -> &[String] {
        match provider {
            Provider::Apple => &self.config.apple_client_ids,
            Provider::Google => &self.config.google_client_ids,
        }
    }

    /// The provider's signing key with this id, fetching the published set when it is old
    /// or doesn't have it (keys rotate).
    async fn key(&self, provider: Provider, kid: &str, now: i64) -> Result<Option<DecodingKey>, AuthError> {
        let cached = self.keys.lock().unwrap().get(&provider).and_then(|(set, at)| {
            (now - at < KEYS_TTL_SECS).then(|| set.find(kid).and_then(|k| DecodingKey::from_jwk(k).ok()))
        });
        if let Some(Some(key)) = cached {
            return Ok(Some(key));
        }
        let url = match provider {
            Provider::Apple => &self.config.apple_keys_url,
            Provider::Google => &self.config.google_keys_url,
        };
        let set: JwkSet = async { self.http.get(url).send().await?.error_for_status()?.json().await }
            .await
            .map_err(|e: reqwest::Error| AuthError::Upstream(provider.name(), e.into()))?;
        let key = set.find(kid).and_then(|k| DecodingKey::from_jwk(k).ok());
        self.keys.lock().unwrap().insert(provider, (set, now));
        Ok(key)
    }

    /// Checks an ID token's signature, issuer, audience and expiry.
    pub async fn verify(&self, provider: Provider, token: &str, now: i64) -> Result<Verified, AuthError> {
        if self.audiences(provider).is_empty() {
            return Err(AuthError::NotConfigured(provider.name()));
        }
        let header = decode_header(token).map_err(|_| AuthError::Invalid)?;
        let kid = header.kid.ok_or(AuthError::Invalid)?;
        let key = self.key(provider, &kid, now).await?.ok_or(AuthError::Invalid)?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(self.audiences(provider));
        validation.set_issuer(provider.issuers());
        let claims = decode::<Claims>(token, &key, &validation).map_err(|_| AuthError::Invalid)?.claims;
        let verified = matches!(&claims.email_verified, Some(serde_json::Value::Bool(true)))
            || matches!(&claims.email_verified, Some(serde_json::Value::String(s)) if s == "true");
        Ok(Verified {
            subject: claims.sub,
            email: claims.email.filter(|_| verified).map(|e| e.to_lowercase()),
        })
    }

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
        && !email.chars().any(|c| c.is_whitespace() || c == ',' || c == '<' || c == '>');
    ok.then_some(email)
}

fn code_hash(email: &str, code: &str) -> String {
    let digest = Sha256::digest(format!("{email}:{code}").as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
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
    let code: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
    let Some(row) = sqlx::query("SELECT hash, expires_at, attempts FROM email_codes WHERE email = ?")
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
    sqlx::query("DELETE FROM email_codes WHERE email = ?").bind(email).execute(db).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_are_normalized_and_checked() {
        assert_eq!(normalized_email("  Jan@Example.COM "), Some("jan@example.com".into()));
        assert_eq!(normalized_email("no-at-sign"), None);
        assert_eq!(normalized_email("a@localhost"), None);
        assert_eq!(normalized_email("a@b.c, x@y.z"), None);
    }
}
