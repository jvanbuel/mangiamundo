//! Firebase Cloud Messaging (HTTP v1), for the Android app.
//!
//! Each request carries an OAuth access token, which a service account gets by signing a
//! JWT with its private key. Tokens last an hour; a new one is fetched shortly before.

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use super::{Delivery, Notification};

const SEND_URL: &str = "https://fcm.googleapis.com";
const SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
/// The Android notification channel the app creates for these.
pub const CHANNEL: &str = "new_food";

pub struct Fcm {
    http: reqwest::Client,
    project_id: String,
    client_email: String,
    key: EncodingKey,
    token_uri: String,
    send_url: String,
    access: Mutex<Option<(String, i64)>>,
}

/// The parts of a service account key file (`FCM_SERVICE_ACCOUNT`) that are needed.
#[derive(Deserialize)]
struct ServiceAccount {
    project_id: String,
    client_email: String,
    private_key: String,
    #[serde(default = "default_token_uri")]
    token_uri: String,
}

fn default_token_uri() -> String {
    "https://oauth2.googleapis.com/token".into()
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    iat: i64,
    exp: i64,
}

#[derive(Deserialize)]
struct AccessToken {
    access_token: String,
    expires_in: i64,
}

impl Fcm {
    /// `service_account` is the contents of the JSON key file.
    pub fn new(http: reqwest::Client, service_account: &str) -> anyhow::Result<Self> {
        let account: ServiceAccount = serde_json::from_str(service_account)
            .map_err(|e| anyhow::anyhow!("FCM_SERVICE_ACCOUNT is not a service account key: {e}"))?;
        let key = EncodingKey::from_rsa_pem(account.private_key.as_bytes())
            .map_err(|e| anyhow::anyhow!("FCM_SERVICE_ACCOUNT has no usable private key: {e}"))?;
        Ok(Self {
            http,
            project_id: account.project_id,
            client_email: account.client_email,
            key,
            token_uri: account.token_uri,
            send_url: SEND_URL.into(),
            access: Mutex::new(None),
        })
    }

    /// Sends to another server instead, for tests.
    pub fn with_url(mut self, url: &str) -> Self {
        self.send_url = url.into();
        self
    }

    async fn access_token(&self, now: i64) -> anyhow::Result<String> {
        let mut access = self.access.lock().await;
        if let Some((token, expires)) = access.as_ref() {
            if now < expires - 300 {
                return Ok(token.clone());
            }
        }
        let claims = Claims { iss: &self.client_email, scope: SCOPE, aud: &self.token_uri, iat: now, exp: now + 3600 };
        let assertion = jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &self.key)?;
        let token: AccessToken = self
            .http
            .post(&self.token_uri)
            .form(&[("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"), ("assertion", &assertion)])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        *access = Some((token.access_token.clone(), now + token.expires_in));
        Ok(token.access_token)
    }

    pub async fn send(&self, device: &str, notification: &Notification, now: i64) -> anyhow::Result<Delivery> {
        let message = serde_json::json!({
            "message": {
                "token": device,
                "notification": { "title": notification.title, "body": notification.body },
                "data": { "url": notification.url },
                "android": {
                    "priority": "high",
                    "ttl": "43200s",
                    "collapse_key": notification.tag,
                    "notification": { "tag": notification.tag, "channel_id": CHANNEL },
                },
            }
        });
        let response = self
            .http
            .post(format!("{}/v1/projects/{}/messages:send", self.send_url, self.project_id))
            .bearer_auth(self.access_token(now).await?)
            .json(&message)
            .send()
            .await?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(Delivery::Sent);
        }
        let body = response.text().await.unwrap_or_default();
        // An uninstalled app answers 404 UNREGISTERED; a mangled token 400 about the token.
        if status == 404 || (status == 400 && body.contains("registration token")) {
            return Ok(Delivery::Gone);
        }
        anyhow::bail!("FCM answered {status}: {body}")
    }
}
