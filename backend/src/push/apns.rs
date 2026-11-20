//! Apple Push Notification service, for the iOS app.
//!
//! Requests are signed with a token (an ES256 JWT) made from the team's `.p8` key. Apple
//! wants the same token reused for up to an hour and refuses new ones more often than
//! every 20 minutes, so it is kept and renewed after 40.

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;
use tokio::sync::Mutex;

use super::{Delivery, Notification};

const PRODUCTION: &str = "https://api.push.apple.com";
const SANDBOX: &str = "https://api.sandbox.push.apple.com";
const TOKEN_LIFETIME_SECS: i64 = 40 * 60;

pub struct Apns {
    http: reqwest::Client,
    key: EncodingKey,
    key_id: String,
    team_id: String,
    /// The app's bundle id.
    topic: String,
    production_url: String,
    sandbox_url: String,
    token: Mutex<Option<(String, i64)>>,
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    iat: i64,
}

impl Apns {
    /// `key` is the contents of the `.p8` file.
    pub fn new(http: reqwest::Client, key: &str, key_id: String, team_id: String, topic: String) -> anyhow::Result<Self> {
        let key = EncodingKey::from_ec_pem(key.replace("\\n", "\n").as_bytes())
            .map_err(|e| anyhow::anyhow!("APNS_KEY is not a .p8 key: {e}"))?;
        Ok(Self {
            http,
            key,
            key_id,
            team_id,
            topic,
            production_url: PRODUCTION.into(),
            sandbox_url: SANDBOX.into(),
            token: Mutex::new(None),
        })
    }

    /// Sends to another server instead, for tests.
    pub fn with_url(mut self, url: &str) -> Self {
        self.production_url = url.into();
        self.sandbox_url = url.into();
        self
    }

    async fn token(&self, now: i64) -> anyhow::Result<String> {
        let mut token = self.token.lock().await;
        if let Some((jwt, at)) = token.as_ref() {
            if now - at < TOKEN_LIFETIME_SECS {
                return Ok(jwt.clone());
            }
        }
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.key_id.clone());
        let jwt = jsonwebtoken::encode(&header, &Claims { iss: &self.team_id, iat: now }, &self.key)?;
        *token = Some((jwt.clone(), now));
        Ok(jwt)
    }

    pub async fn send(&self, device: &str, sandbox: bool, notification: &Notification, now: i64) -> anyhow::Result<Delivery> {
        let base = if sandbox { &self.sandbox_url } else { &self.production_url };
        let payload = serde_json::json!({
            "aps": {
                "alert": { "title": notification.title, "body": notification.body },
                "sound": "default",
            },
            "url": notification.url,
        });
        // A collapse id is at most 64 bytes.
        let collapse: String = notification.tag.chars().take(64).collect();
        let response = self
            .http
            .post(format!("{base}/3/device/{device}"))
            .bearer_auth(self.token(now).await?)
            .header("apns-topic", &self.topic)
            .header("apns-push-type", "alert")
            .header("apns-priority", "10")
            .header("apns-expiration", (now + 12 * 3600).to_string())
            .header("apns-collapse-id", collapse)
            .json(&payload)
            .send()
            .await?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(Delivery::Sent);
        }
        let body = response.text().await.unwrap_or_default();
        let reason = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v["reason"].as_str().map(String::from))
            .unwrap_or_default();
        match (status, reason.as_str()) {
            (410, _) | (400, "BadDeviceToken" | "DeviceTokenNotForTopic") => Ok(Delivery::Gone),
            _ => anyhow::bail!("APNs answered {status}: {body}"),
        }
    }
}
