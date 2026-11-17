use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProviderKind {
    Osm,
    Demo,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: String,
    pub database_url: String,
    pub provider: ProviderKind,
    /// Overpass servers, tried in order until one answers.
    pub overpass_urls: Vec<String>,
    pub nominatim_url: String,
    /// How long a cached area stays fresh before the provider is asked again.
    pub cache_ttl_secs: i64,
    /// Directory with the built web app, served at `/` when it exists.
    pub static_dir: Option<PathBuf>,
    /// Raw P-256 private key (base64url) for Web Push. Generated and stored when unset.
    pub vapid_private_key: Option<String>,
    /// Contact sent to push services: a `mailto:` or `https:` URL.
    pub vapid_subject: String,
    /// Seconds between passes over push subscriptions; 0 turns reminders off.
    pub push_interval_secs: u64,
    /// Where closed places are reported to OpenStreetMap. Unset in development, so
    /// testing locally never posts real notes.
    pub osm_notes_url: Option<String>,
}

/// The main public instance is often overloaded, so community mirrors back it up.
const DEFAULT_OVERPASS_URLS: &str = "https://overpass-api.de/api/interpreter,\
    https://overpass.private.coffee/api/interpreter,\
    https://overpass.kumi.systems/api/interpreter";

fn var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let provider = match var("PLACES_PROVIDER").as_deref() {
            Some("osm") | None => ProviderKind::Osm,
            Some("demo") => ProviderKind::Demo,
            Some("google") => anyhow::bail!(
                "PLACES_PROVIDER=google is no longer supported: areas are postal codes and cities, \
                 and Google has no postal-code boundaries. Use osm or demo."
            ),
            Some(other) => anyhow::bail!("unknown PLACES_PROVIDER {other:?} (osm or demo)"),
        };
        Ok(Self {
            bind: var("BIND").unwrap_or_else(|| "0.0.0.0:8080".into()),
            database_url: var("DATABASE_URL").unwrap_or_else(|| "sqlite://mangiamundo.db".into()),
            provider,
            overpass_urls: var("OVERPASS_URLS")
                .unwrap_or_else(|| DEFAULT_OVERPASS_URLS.into())
                .split(',')
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty())
                .collect(),
            nominatim_url: var("NOMINATIM_URL")
                .unwrap_or_else(|| "https://nominatim.openstreetmap.org".into()),
            cache_ttl_secs: var("CACHE_TTL_DAYS")
                .map(|v| v.parse::<i64>())
                .transpose()?
                .unwrap_or(7)
                * 86_400,
            vapid_private_key: var("VAPID_PRIVATE_KEY"),
            vapid_subject: var("VAPID_SUBJECT").unwrap_or_else(|| "mailto:hello@mangiamundo.app".into()),
            osm_notes_url: var("OSM_NOTES_URL"),
            push_interval_secs: var("PUSH_INTERVAL_SECS").map(|v| v.parse()).transpose()?.unwrap_or(600),
            static_dir: Some(PathBuf::from(
                var("STATIC_DIR").unwrap_or_else(|| "../web/build".into()),
            )),
        })
    }
}
