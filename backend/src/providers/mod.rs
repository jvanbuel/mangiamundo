//! Sources of restaurant data and of the areas (cities and postal codes) they are grouped in.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;

use crate::config::{Config, ProviderKind};
use crate::geo;

pub mod demo;
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
    /// In OSM `opening_hours` syntax.
    pub opening_hours: Option<String>,
    /// ISO alpha-2 codes of the countries whose cuisine this place serves.
    pub countries: Vec<&'static str>,
}

#[async_trait]
pub trait PlacesProvider: Send + Sync {
    fn name(&self) -> &'static str;
    /// All places serving a national cuisine inside the area's outline. `on_next_server` is
    /// called when the provider starts asking another server, for the progress wheel.
    async fn search(&self, area: &Boundary, on_next_server: &(dyn Fn() + Send + Sync)) -> anyhow::Result<Vec<FoundPlace>>;
    /// About how long `search` takes for this area, for the progress wheel.
    fn expected_secs(&self, _area: &Boundary) -> f64 {
        20.0
    }
    /// One place read again from the source, by its id.
    async fn place(&self, _id: &str) -> anyhow::Result<Lookup> {
        anyhow::bail!("{} places can't be read one at a time", self.name())
    }
    /// Places serving a national cuisine within `radius_m` of a point.
    async fn nearby(&self, _lat: f64, _lng: f64, _radius_m: f64) -> anyhow::Result<Vec<FoundPlace>> {
        anyhow::bail!("{} can't search around a point", self.name())
    }
}

/// A place read again from its source.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup {
    Found(FoundPlace),
    /// Deleted, closed, no longer a restaurant, or its cuisine maps to no country.
    Gone,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AreaKind {
    /// A city, town or municipality: every postal code in it.
    City,
    PostalCode,
}

/// A place people pick as their location: a city or a postal code.
#[derive(Debug, Clone, Serialize, PartialEq, utoipa::ToSchema)]
pub struct AreaInfo {
    /// Opaque and URL-safe, prefixed with the directory name, e.g. `osm-relation-196311`.
    pub id: String,
    /// "Leuven", or "3000 Leuven" for a postal code.
    pub name: String,
    pub kind: AreaKind,
    /// Where it is, to tell same-named areas apart: "Vlaams-Brabant, Belgium".
    #[schema(required = true)]
    pub detail: Option<String>,
    /// A central point, for the map.
    pub lat: f64,
    pub lng: f64,
}

/// An area with its outline.
#[derive(Debug, Clone, PartialEq)]
pub struct Boundary {
    pub info: AreaInfo,
    pub shape: geo::Shape,
}

/// The areas a point lies in.
#[derive(Debug, Clone, Default, Serialize, PartialEq, utoipa::ToSchema)]
pub struct Located {
    #[schema(required = true)]
    pub city: Option<AreaInfo>,
    #[schema(required = true)]
    pub postal_code: Option<AreaInfo>,
}

#[async_trait]
pub trait AreaDirectory: Send + Sync {
    /// Cities and postal codes matching what someone typed.
    async fn search(&self, query: &str) -> anyhow::Result<Vec<AreaInfo>>;
    /// The city and postal code a point lies in.
    async fn locate(&self, lat: f64, lng: f64) -> anyhow::Result<Located>;
    /// An area's outline, or `None` when the id is not a city or postal code.
    async fn boundary(&self, id: &str) -> anyhow::Result<Option<Boundary>>;
    /// Quick suggestions while someone types, nearest first when `near` is known.
    async fn suggest(&self, query: &str, _near: Option<(f64, f64)>) -> anyhow::Result<Vec<AreaInfo>> {
        self.search(query).await
    }
}

pub fn build(config: &Config) -> anyhow::Result<(Arc<dyn PlacesProvider>, Arc<dyn AreaDirectory>)> {
    let http = reqwest::Client::builder()
        .user_agent(concat!("mangiamundo/", env!("CARGO_PKG_VERSION")))
        // Fail over quickly from a server that is down, but allow slow Overpass queries.
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(75))
        .build()?;

    Ok(match config.provider {
        ProviderKind::Demo => (Arc::new(demo::Demo), Arc::new(demo::Demo)),
        ProviderKind::Osm => (
            Arc::new(osm::Overpass::new(http.clone(), config.overpass_urls.clone())),
            Arc::new(osm::Nominatim::new(http, config.nominatim_url.clone())),
        ),
    })
}
