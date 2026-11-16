//! Google Maps Platform: Places API (New) Nearby Search and the Geocoding API.
//!
//! Google has no free-form cuisine field, only a fixed list of `*_restaurant` place
//! types. One Nearby Search request is made per type (each returns at most 20 places),
//! so only the cuisines that have a `google` type in the catalog can be discovered.

use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

use super::{FoundPlace, GeoResult, Geocoder, PlacesProvider};
use crate::catalog::{Cuisine, CUISINES};

const NEARBY_URL: &str = "https://places.googleapis.com/v1/places:searchNearby";
const GEOCODE_URL: &str = "https://maps.googleapis.com/maps/api/geocode/json";
const FIELD_MASK: &str = "places.id,places.displayName,places.location,places.formattedAddress,\
    places.googleMapsUri,places.regularOpeningHours";
/// Nearby Search rejects a larger radius.
const MAX_RADIUS_M: u32 = 50_000;
const CONCURRENCY: usize = 6;

pub struct GooglePlaces {
    http: reqwest::Client,
    key: String,
}

impl GooglePlaces {
    pub fn new(http: reqwest::Client, key: String) -> Self {
        Self { http, key }
    }

    async fn search_type(
        &self,
        place_type: &str,
        lat: f64,
        lng: f64,
        radius_m: u32,
    ) -> anyhow::Result<Vec<NearbyPlace>> {
        let response = self
            .http
            .post(NEARBY_URL)
            .header("X-Goog-Api-Key", &self.key)
            .header("X-Goog-FieldMask", FIELD_MASK)
            .json(&json!({
                "includedPrimaryTypes": [place_type],
                "maxResultCount": 20,
                "rankPreference": "POPULARITY",
                "locationRestriction": {
                    "circle": {
                        "center": { "latitude": lat, "longitude": lng },
                        "radius": radius_m.min(MAX_RADIUS_M),
                    }
                }
            }))
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("{status}: {}", response.text().await.unwrap_or_default());
        }
        Ok(response.json::<NearbyResponse>().await?.places)
    }
}

impl GooglePlaces {
    async fn search_cuisine(
        &self,
        cuisine: &'static Cuisine,
        place_type: &'static str,
        lat: f64,
        lng: f64,
        radius_m: u32,
    ) -> (&'static Cuisine, &'static str, anyhow::Result<Vec<NearbyPlace>>) {
        (cuisine, place_type, self.search_type(place_type, lat, lng, radius_m).await)
    }
}

#[derive(Deserialize)]
struct NearbyResponse {
    #[serde(default)]
    places: Vec<NearbyPlace>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NearbyPlace {
    id: String,
    display_name: Option<DisplayName>,
    location: Location,
    formatted_address: Option<String>,
    google_maps_uri: Option<String>,
    regular_opening_hours: Option<OpeningHours>,
}

#[derive(Deserialize)]
struct OpeningHours {
    #[serde(default)]
    periods: Vec<Period>,
}

#[derive(Deserialize)]
struct Period {
    open: Point,
    close: Option<Point>,
}

#[derive(Deserialize)]
struct Point {
    /// 0 is Sunday.
    day: usize,
    hour: u32,
    minute: u32,
}

/// Converts Google's weekly periods to OSM syntax, e.g. "Mo 11:00-14:00,18:00-22:00; Tu …".
/// A period that runs past midnight ends at a time before it starts, which OSM reads
/// as continuing into the next day.
fn osm_hours(hours: &OpeningHours) -> Option<String> {
    const DAYS: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
    // Google's way of saying "always open": one period that opens and never closes.
    if hours.periods.iter().any(|p| p.close.is_none()) {
        return Some("24/7".into());
    }
    let mut by_day: Vec<Vec<String>> = vec![Vec::new(); 7];
    for period in &hours.periods {
        let close = period.close.as_ref()?;
        let end = match (close.hour, close.minute) {
            (0, 0) => "24:00".to_string(),
            (h, m) => format!("{h:02}:{m:02}"),
        };
        by_day.get_mut(period.open.day)?.push(format!("{:02}:{:02}-{end}", period.open.hour, period.open.minute));
    }
    // Monday first, as OSM mappers write it.
    let rules: Vec<String> = [1, 2, 3, 4, 5, 6, 0]
        .into_iter()
        .filter(|&d| !by_day[d].is_empty())
        .map(|d| format!("{} {}", DAYS[d], by_day[d].join(",")))
        .collect();
    (!rules.is_empty()).then(|| rules.join("; "))
}

#[derive(Deserialize)]
struct DisplayName {
    text: String,
}

#[derive(Deserialize)]
struct Location {
    latitude: f64,
    longitude: f64,
}

#[async_trait]
impl PlacesProvider for GooglePlaces {
    fn name(&self) -> &'static str {
        "google"
    }

    async fn search(&self, lat: f64, lng: f64, radius_m: u32) -> anyhow::Result<Vec<FoundPlace>> {
        let requests: Vec<_> = CUISINES
            .iter()
            .filter_map(|cuisine| Some(self.search_cuisine(cuisine, cuisine.google?, lat, lng, radius_m)))
            .collect();
        let results: Vec<_> = stream::iter(requests).buffer_unordered(CONCURRENCY).collect().await;

        let mut places: HashMap<String, FoundPlace> = HashMap::new();
        let (mut ok, mut last_error) = (0, None);
        for (cuisine, place_type, result) in results {
            match result {
                Ok(found) => {
                    ok += 1;
                    for p in found {
                        let Some(name) = p.display_name else { continue };
                        let entry = places.entry(p.id.clone()).or_insert_with(|| FoundPlace {
                            id: format!("google-{}", p.id),
                            name: name.text,
                            lat: p.location.latitude,
                            lng: p.location.longitude,
                            address: p.formatted_address,
                            url: p.google_maps_uri,
                            cuisine: Some(cuisine.osm[0].to_string()),
                            opening_hours: p.regular_opening_hours.as_ref().and_then(osm_hours),
                            countries: Vec::new(),
                        });
                        if !entry.countries.contains(&cuisine.country) {
                            entry.countries.push(cuisine.country);
                        }
                    }
                }
                // A single unknown type or transient error must not hide every other cuisine.
                Err(error) => {
                    tracing::warn!(place_type, %error, "google nearby search failed");
                    last_error = Some(error);
                }
            }
        }
        match (ok, last_error) {
            (0, Some(error)) => Err(error.context("every Google Places request failed")),
            _ => Ok(places.into_values().collect()),
        }
    }
}

pub struct GoogleGeocoder {
    http: reqwest::Client,
    key: String,
}

impl GoogleGeocoder {
    pub fn new(http: reqwest::Client, key: String) -> Self {
        Self { http, key }
    }

    async fn call(&self, params: &[(&str, &str)]) -> anyhow::Result<GeocodeResponse> {
        let response: GeocodeResponse = self
            .http
            .get(GEOCODE_URL)
            .query(params)
            .query(&[("key", self.key.as_str())])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        match response.status.as_str() {
            "OK" | "ZERO_RESULTS" => Ok(response),
            other => anyhow::bail!(
                "geocoding failed: {other} {}",
                response.error_message.as_deref().unwrap_or_default()
            ),
        }
    }
}

#[derive(Deserialize)]
struct GeocodeResponse {
    status: String,
    error_message: Option<String>,
    #[serde(default)]
    results: Vec<GeocodeResult>,
}

#[derive(Deserialize)]
struct GeocodeResult {
    formatted_address: String,
    geometry: Geometry,
    #[serde(default)]
    address_components: Vec<AddressComponent>,
}

#[derive(Deserialize)]
struct Geometry {
    location: LatLng,
}

#[derive(Deserialize)]
struct LatLng {
    lat: f64,
    lng: f64,
}

#[derive(Deserialize)]
struct AddressComponent {
    long_name: String,
    types: Vec<String>,
}

#[async_trait]
impl Geocoder for GoogleGeocoder {
    async fn search(&self, query: &str) -> anyhow::Result<Vec<GeoResult>> {
        let response = self.call(&[("address", query)]).await?;
        Ok(response
            .results
            .into_iter()
            .take(5)
            .map(|r| GeoResult {
                name: r.formatted_address,
                lat: r.geometry.location.lat,
                lng: r.geometry.location.lng,
            })
            .collect())
    }

    async fn reverse(&self, lat: f64, lng: f64) -> anyhow::Result<Option<String>> {
        let latlng = format!("{lat},{lng}");
        let response = self
            .call(&[("latlng", &latlng), ("result_type", "locality|postal_town")])
            .await?;
        Ok(response.results.into_iter().next().map(|r| {
            r.address_components
                .into_iter()
                .find(|c| c.types.iter().any(|t| t == "locality" || t == "postal_town"))
                .map(|c| c.long_name)
                .unwrap_or(r.formatted_address)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hours(json: &str) -> Option<String> {
        osm_hours(&serde_json::from_str(json).unwrap())
    }

    #[test]
    fn converts_periods_to_osm_syntax() {
        let json = r#"{"periods":[
            {"open":{"day":1,"hour":11,"minute":30},"close":{"day":1,"hour":14,"minute":0}},
            {"open":{"day":1,"hour":18,"minute":0},"close":{"day":2,"hour":0,"minute":0}},
            {"open":{"day":5,"hour":18,"minute":0},"close":{"day":6,"hour":2,"minute":0}},
            {"open":{"day":0,"hour":12,"minute":0},"close":{"day":0,"hour":15,"minute":0}}
        ]}"#;
        assert_eq!(hours(json).as_deref(), Some("Mo 11:30-14:00,18:00-24:00; Fr 18:00-02:00; Su 12:00-15:00"));
    }

    #[test]
    fn always_open_and_empty() {
        assert_eq!(hours(r#"{"periods":[{"open":{"day":0,"hour":0,"minute":0}}]}"#).as_deref(), Some("24/7"));
        assert_eq!(hours(r#"{"periods":[]}"#), None);
    }
}
