use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProviderKind {
    Osm,
    Google,
    Demo,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: String,
    pub database_url: String,
    pub provider: ProviderKind,
    pub google_api_key: Option<String>,
    pub overpass_url: String,
    pub nominatim_url: String,
    /// How long a cached area stays fresh before the provider is asked again.
    pub cache_ttl_secs: i64,
    /// Directory with the built web app, served at `/` when it exists.
    pub static_dir: Option<PathBuf>,
}

fn var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let google_api_key = var("GOOGLE_MAPS_API_KEY");
        let provider = match var("PLACES_PROVIDER").as_deref() {
            Some("osm") => ProviderKind::Osm,
            Some("google") => ProviderKind::Google,
            Some("demo") => ProviderKind::Demo,
            Some(other) => anyhow::bail!("unknown PLACES_PROVIDER {other:?} (osm, google or demo)"),
            None if google_api_key.is_some() => ProviderKind::Google,
            None => ProviderKind::Osm,
        };
        Ok(Self {
            bind: var("BIND").unwrap_or_else(|| "0.0.0.0:8080".into()),
            database_url: var("DATABASE_URL").unwrap_or_else(|| "sqlite://mangiamundo.db".into()),
            provider,
            google_api_key,
            overpass_url: var("OVERPASS_URL")
                .unwrap_or_else(|| "https://overpass-api.de/api/interpreter".into()),
            nominatim_url: var("NOMINATIM_URL")
                .unwrap_or_else(|| "https://nominatim.openstreetmap.org".into()),
            cache_ttl_secs: var("CACHE_TTL_DAYS")
                .map(|v| v.parse::<i64>())
                .transpose()?
                .unwrap_or(7)
                * 86_400,
            static_dir: Some(PathBuf::from(
                var("STATIC_DIR").unwrap_or_else(|| "../web/build".into()),
            )),
        })
    }
}
