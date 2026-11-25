//! Signing in with Apple, Google or a one-time code sent by email. The apps get an ID token
//! from Apple or Google and send it here; its signature is checked against the provider's
//! published keys, so no third-party auth service is involved.

mod email;
mod tokens;

use std::collections::HashMap;
use std::sync::Mutex;

use jsonwebtoken::jwk::JwkSet;

pub use email::{new_code, normalized_email, take_code};

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
        Auth {
            config,
            ..Default::default()
        }
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
    /// Google's `profile` scope: the person's name and a photo link. Apple shares neither.
    pub name: Option<String>,
    pub picture: Option<String>,
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
