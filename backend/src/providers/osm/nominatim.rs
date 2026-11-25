//! Nominatim: cities and postal codes, one request per second.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;

use super::results::{area_info, photon_areas, results, text, CITY_TYPES};
use crate::geo::Shape;
use crate::providers::{AreaDirectory, AreaInfo, Boundary, Located, Refused};

/// Nominatim's usage policy allows one request per second.
const NOMINATIM_INTERVAL: Duration = Duration::from_secs(1);

/// How long a request waits for its turn. Those ahead of it take a second each, so this
/// is also about how many can be waiting; the next one is told to try again.
const NOMINATIM_PATIENCE: Duration = Duration::from_secs(10);

/// How long Nominatim gets to answer, as everyone else waits meanwhile. It usually takes
/// well under a second.
const NOMINATIM_TIMEOUT: Duration = Duration::from_secs(15);

pub struct Nominatim {
    pub(super) http: reqwest::Client,
    pub(super) url: String,
    /// When the last request went out; held during a request so they never overlap.
    pub(super) last: tokio::sync::Mutex<Option<Instant>>,
    pub(super) patience: Duration,
    pub(super) timeout: Duration,
}

impl Nominatim {
    pub fn new(http: reqwest::Client, url: String) -> Self {
        Self {
            http,
            url,
            last: tokio::sync::Mutex::new(None),
            patience: NOMINATIM_PATIENCE,
            timeout: NOMINATIM_TIMEOUT,
        }
    }

    async fn get(&self, path: &str, params: &[(&str, &str)]) -> anyhow::Result<Value> {
        // Turns are given in the order they were asked for.
        let Ok(mut last) = tokio::time::timeout(self.patience, self.last.lock()).await else {
            return Err(Refused::Busy.into());
        };
        if let Some(at) = *last {
            tokio::time::sleep(NOMINATIM_INTERVAL.saturating_sub(at.elapsed())).await;
        }
        *last = Some(Instant::now());
        let response = self
            .http
            .get(format!("{}/{path}", self.url))
            .query(&[
                ("format", "jsonv2"),
                ("addressdetails", "1"),
                ("accept-language", "en"),
            ])
            .query(params)
            .timeout(self.timeout)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(response)
    }

    /// Structured search for one kind of area around a point; the one containing the point wins.
    async fn containing(
        &self,
        params: &[(&str, &str)],
        lat: f64,
        lng: f64,
    ) -> anyhow::Result<Option<AreaInfo>> {
        let found = self
            .get(
                "search",
                &[
                    params,
                    &[("polygon_geojson", "1"), ("polygon_threshold", "0.001")],
                ]
                .concat(),
            )
            .await?;
        let area = results(&found).find_map(|r| {
            let info = area_info(r)?;
            Shape::from_geojson(r.get("geojson")?)?
                .contains(lat, lng)
                .then_some(info)
        });
        Ok(area)
    }
}

#[async_trait]
impl AreaDirectory for Nominatim {
    async fn search(&self, query: &str) -> anyhow::Result<Vec<AreaInfo>> {
        let found = self.get("search", &[("q", query), ("limit", "10")]).await?;
        let areas: Vec<AreaInfo> = results(&found).filter_map(area_info).collect();
        if !areas.is_empty() {
            return Ok(areas);
        }
        // A street or a landmark: offer the city and postal code it lies in.
        let Some(first) = results(&found).next() else {
            return Ok(Vec::new());
        };
        let (Some(lat), Some(lng)) = (
            text(first, "lat").and_then(|v| v.parse().ok()),
            text(first, "lon").and_then(|v| v.parse().ok()),
        ) else {
            return Ok(Vec::new());
        };
        let located = self.locate(lat, lng).await?;
        Ok(located
            .city
            .into_iter()
            .chain(located.postal_code)
            .collect())
    }

    async fn locate(&self, lat: f64, lng: f64) -> anyhow::Result<Located> {
        // Reverse geocoding names the city and postal code, but returns the smallest
        // boundary (often a district), so look both up by name.
        let (lat_s, lng_s) = (lat.to_string(), lng.to_string());
        let here = self
            .get(
                "reverse",
                &[("lat", &lat_s), ("lon", &lng_s), ("zoom", "18")],
            )
            .await?;
        let address = here.get("address").cloned().unwrap_or_default();
        let Some(country) = text(&address, "country_code") else {
            return Ok(Located::default());
        };
        let mut located = Located::default();
        if let Some(city) = CITY_TYPES.iter().find_map(|k| text(&address, k)) {
            // The state narrows common names (Springfield) so the right one is among the
            // results; without it as a fallback, in case the names don't line up.
            let by_name = [("city", city), ("countrycodes", country)];
            if let Some(state) = text(&address, "state") {
                located.city = self
                    .containing(&[&by_name[..], &[("state", state)]].concat(), lat, lng)
                    .await?;
            }
            if located.city.is_none() {
                located.city = self.containing(&by_name, lat, lng).await?;
            }
        }
        if let Some(code) = text(&address, "postcode") {
            located.postal_code = self
                .containing(&[("postalcode", code), ("countrycodes", country)], lat, lng)
                .await?;
        }
        Ok(located)
    }

    async fn boundary(&self, id: &str) -> anyhow::Result<Option<Boundary>> {
        let Some(relation) = id
            .strip_prefix("osm-relation-")
            .filter(|n| n.bytes().all(|b| b.is_ascii_digit()) && !n.is_empty())
        else {
            return Ok(None);
        };
        let ids = format!("R{relation}");
        let found = self
            .get(
                "lookup",
                &[
                    ("osm_ids", &ids),
                    ("polygon_geojson", "1"),
                    ("polygon_threshold", "0.0003"),
                ],
            )
            .await?;
        let boundary = results(&found).next().and_then(|r| {
            Some(Boundary {
                info: area_info(r)?,
                shape: Shape::from_geojson(r.get("geojson")?)?,
            })
        });
        Ok(boundary)
    }

    /// Nominatim's usage policy forbids search-as-you-type, so suggestions come from
    /// Photon, which is built for it and uses the same OSM ids.
    async fn suggest(
        &self,
        query: &str,
        near: Option<(f64, f64)>,
    ) -> anyhow::Result<Vec<AreaInfo>> {
        let mut request =
            self.http
                .get(PHOTON_URL)
                .query(&[("q", query), ("limit", "15"), ("lang", "en")]);
        if let Some((lat, lng)) = near {
            request = request.query(&[("lat", lat), ("lon", lng)]);
        }
        let found: Value = request.send().await?.error_for_status()?.json().await?;
        Ok(photon_areas(&found))
    }
}

const PHOTON_URL: &str = "https://photon.komoot.io/api/";
