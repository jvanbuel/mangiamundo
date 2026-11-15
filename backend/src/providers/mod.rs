//! Sources of restaurant and geocoding data.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;

use crate::config::{Config, ProviderKind};

pub mod demo;
pub mod google;
pub mod osm;

/// A restaurant as returned by a provider, before it is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundPlace {
    /// Globally unique and URL-safe, prefixed with the provider name.
    pub id: String,
    pub name: String,
    pub lat: f64,
    pub lng: f64,
    pub address: Option<String>,
    pub url: Option<String>,
    pub cuisine: Option<String>,
    /// ISO alpha-2 codes of the countries whose cuisine this place serves.
    pub countries: Vec<&'static str>,
}

#[async_trait]
pub trait PlacesProvider: Send + Sync {
    fn name(&self) -> &'static str;
    /// All places serving a national cuisine within `radius_m` of the point.
    async fn search(&self, lat: f64, lng: f64, radius_m: u32) -> anyhow::Result<Vec<FoundPlace>>;
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GeoResult {
    pub name: String,
    pub lat: f64,
    pub lng: f64,
}

#[async_trait]
pub trait Geocoder: Send + Sync {
    async fn search(&self, query: &str) -> anyhow::Result<Vec<GeoResult>>;
    async fn reverse(&self, lat: f64, lng: f64) -> anyhow::Result<Option<String>>;
}

pub fn build(config: &Config) -> anyhow::Result<(Arc<dyn PlacesProvider>, Arc<dyn Geocoder>)> {
    let http = reqwest::Client::builder()
        .user_agent(concat!("mangiamundo/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(90))
        .build()?;

    Ok(match config.provider {
        ProviderKind::Demo => (Arc::new(demo::Demo), Arc::new(demo::Demo)),
        ProviderKind::Osm => (
            Arc::new(osm::Overpass::new(http.clone(), config.overpass_url.clone())),
            Arc::new(osm::Nominatim::new(http, config.nominatim_url.clone())),
        ),
        ProviderKind::Google => {
            let key = config
                .google_api_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("PLACES_PROVIDER=google needs GOOGLE_MAPS_API_KEY"))?;
            (
                Arc::new(google::GooglePlaces::new(http.clone(), key.clone())),
                Arc::new(google::GoogleGeocoder::new(http, key)),
            )
        }
    })
}
