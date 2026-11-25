//! Checking Apple and Google ID tokens against their published keys.

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

use super::{Auth, AuthError, Provider, Verified, KEYS_TTL_SECS};

#[derive(Deserialize)]
struct Claims {
    sub: String,
    email: Option<String>,
    /// A bool from Google, a "true"/"false" string from Apple.
    email_verified: Option<serde_json::Value>,
    name: Option<String>,
    picture: Option<String>,
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
    async fn key(
        &self,
        provider: Provider,
        kid: &str,
        now: i64,
    ) -> Result<Option<DecodingKey>, AuthError> {
        let cached = self
            .keys
            .lock()
            .unwrap()
            .get(&provider)
            .and_then(|(set, at)| {
                (now - at < KEYS_TTL_SECS)
                    .then(|| set.find(kid).and_then(|k| DecodingKey::from_jwk(k).ok()))
            });
        if let Some(Some(key)) = cached {
            return Ok(Some(key));
        }
        let url = match provider {
            Provider::Apple => &self.config.apple_keys_url,
            Provider::Google => &self.config.google_keys_url,
        };
        let set: JwkSet = async {
            self.http
                .get(url)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await
        }
        .await
        .map_err(|e: reqwest::Error| AuthError::Upstream(provider.name(), e.into()))?;
        let key = set.find(kid).and_then(|k| DecodingKey::from_jwk(k).ok());
        self.keys.lock().unwrap().insert(provider, (set, now));
        Ok(key)
    }

    /// Checks an ID token's signature, issuer, audience and expiry.
    pub async fn verify(
        &self,
        provider: Provider,
        token: &str,
        now: i64,
    ) -> Result<Verified, AuthError> {
        if self.audiences(provider).is_empty() {
            return Err(AuthError::NotConfigured(provider.name()));
        }
        let header = decode_header(token).map_err(|_| AuthError::Invalid)?;
        let kid = header.kid.ok_or(AuthError::Invalid)?;
        let key = self
            .key(provider, &kid, now)
            .await?
            .ok_or(AuthError::Invalid)?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(self.audiences(provider));
        validation.set_issuer(provider.issuers());
        let claims = decode::<Claims>(token, &key, &validation)
            .map_err(|_| AuthError::Invalid)?
            .claims;
        let verified = matches!(&claims.email_verified, Some(serde_json::Value::Bool(true)))
            || matches!(&claims.email_verified, Some(serde_json::Value::String(s)) if s == "true");
        Ok(Verified {
            subject: claims.sub,
            email: claims.email.filter(|_| verified).map(|e| e.to_lowercase()),
            name: claims.name.map(|n| n.chars().take(100).collect()),
            // Only a secure link, shown as an image in the app.
            picture: claims
                .picture
                .filter(|p| p.starts_with("https://") && p.len() <= 1000),
        })
    }
}
