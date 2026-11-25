//! Sending one notification to a browser's push service.

use std::time::Duration as StdDuration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sqlx::SqlitePool;
use web_push::{
    request_builder, ContentEncoding, SubscriptionInfo, VapidSignatureBuilder,
    WebPushMessageBuilder,
};

use super::{Notification, Subscription};
use crate::store;

#[derive(Clone)]
pub struct Pusher {
    http: reqwest::Client,
    /// Raw P-256 private key, base64url.
    private_key: String,
    /// Public key for browsers (`applicationServerKey`), base64url.
    pub public_key: String,
    /// Contact for push services: a `mailto:` or `https:` URL.
    subject: String,
}

#[derive(Debug, PartialEq)]
pub enum Delivery {
    Sent,
    /// The browser unsubscribed; the subscription should be forgotten.
    Gone,
}

impl Pusher {
    /// Uses the key in `VAPID_PRIVATE_KEY`, or one stored in the database, creating it on first run.
    pub async fn load(
        db: &SqlitePool,
        configured: Option<String>,
        subject: String,
    ) -> anyhow::Result<Self> {
        let private_key = match configured {
            Some(key) => key,
            None => {
                if let Some(key) = store::setting(db, "vapid_private_key").await? {
                    key
                } else {
                    let key = new_private_key()?;
                    store::set_setting(db, "vapid_private_key", &key).await?;
                    key
                }
            }
        };
        let public_key = URL_SAFE_NO_PAD.encode(
            VapidSignatureBuilder::from_base64_no_sub(&private_key)
                .map_err(|e| anyhow::anyhow!("invalid VAPID key: {e}"))?
                .get_public_key(),
        );
        let http = reqwest::Client::builder()
            .timeout(StdDuration::from_secs(20))
            .build()?;
        Ok(Self {
            http,
            private_key,
            public_key,
            subject,
        })
    }

    pub async fn send(
        &self,
        to: &Subscription,
        notification: &Notification,
    ) -> anyhow::Result<Delivery> {
        let info =
            SubscriptionInfo::new(to.endpoint.as_str(), to.p256dh.as_str(), to.auth.as_str());
        let mut signature = VapidSignatureBuilder::from_base64(&self.private_key, &info)?;
        signature.add_claim("sub", self.subject.as_str());
        let payload = serde_json::to_vec(notification)?;
        let mut message = WebPushMessageBuilder::new(&info);
        message.set_payload(ContentEncoding::Aes128Gcm, &payload);
        message.set_vapid_signature(signature.build()?);
        message.set_ttl(12 * 3600);
        let request: http::Request<Vec<u8>> = request_builder::build_request(message.build()?);

        // web-push speaks an older `http` crate than reqwest; copy the request across.
        let mut outgoing = self.http.post(request.uri().to_string());
        for (name, value) in request.headers() {
            outgoing = outgoing.header(name.as_str(), value.as_bytes());
        }
        let response = outgoing.body(request.into_body()).send().await?;
        match response.status().as_u16() {
            200..=299 => Ok(Delivery::Sent),
            404 | 410 => Ok(Delivery::Gone),
            status => anyhow::bail!(
                "push service answered {status}: {}",
                response.text().await.unwrap_or_default()
            ),
        }
    }
}

fn new_private_key() -> anyhow::Result<String> {
    // Any 32 random bytes are a valid P-256 key except with negligible probability;
    // the builder rejects the rare invalid one, so try again in that case.
    loop {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("no randomness: {e}"))?;
        let key = URL_SAFE_NO_PAD.encode(bytes);
        if VapidSignatureBuilder::from_base64_no_sub(&key).is_ok() {
            return Ok(key);
        }
    }
}
