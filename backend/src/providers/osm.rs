//! OpenStreetMap: Overpass for restaurants, Nominatim for geocoding. No API key needed.

use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;

use super::{FoundPlace, GeoResult, Geocoder, PlacesProvider};
use crate::catalog;

pub struct Overpass {
    http: reqwest::Client,
    urls: Vec<String>,
}

impl Overpass {
    pub fn new(http: reqwest::Client, urls: Vec<String>) -> Self {
        Self { http, urls }
    }

    async fn fetch(&self, url: &str, query: &str) -> anyhow::Result<Vec<FoundPlace>> {
        let body = self
            .http
            .post(url)
            .form(&[("data", query)])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        parse(&body)
    }
}

#[derive(Deserialize)]
struct OverpassResponse {
    elements: Vec<Element>,
}

#[derive(Deserialize)]
struct Element {
    #[serde(rename = "type")]
    kind: String,
    id: u64,
    lat: Option<f64>,
    lon: Option<f64>,
    center: Option<Center>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Deserialize)]
struct Center {
    lat: f64,
    lon: f64,
}

fn query(lat: f64, lng: f64, radius_m: u32) -> String {
    format!(
        "[out:json][timeout:60];\
         nwr[\"amenity\"~\"^(restaurant|fast_food|cafe|pub|bar)$\"][\"cuisine\"][\"name\"]\
         (around:{radius_m},{lat},{lng});\
         out center tags;"
    )
}

fn parse(body: &str) -> anyhow::Result<Vec<FoundPlace>> {
    let response: OverpassResponse = serde_json::from_str(body)?;
    let mut places = Vec::new();
    for el in response.elements {
        let (Some(name), Some(cuisine)) = (el.tags.get("name"), el.tags.get("cuisine")) else {
            continue;
        };
        let countries = catalog::countries_for_cuisine_tag(cuisine);
        if countries.is_empty() {
            continue;
        }
        let (lat, lng) = match (el.lat, el.lon, &el.center) {
            (Some(lat), Some(lon), _) => (lat, lon),
            (_, _, Some(c)) => (c.lat, c.lon),
            _ => continue,
        };
        let street = [el.tags.get("addr:street"), el.tags.get("addr:housenumber")]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ");
        let address = [Some(street.as_str()), el.tags.get("addr:city").map(String::as_str)]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        places.push(FoundPlace {
            id: format!("osm-{}-{}", el.kind, el.id),
            name: name.clone(),
            lat,
            lng,
            address: (!address.is_empty()).then_some(address),
            url: el
                .tags
                .get("website")
                .or_else(|| el.tags.get("contact:website"))
                .cloned(),
            cuisine: Some(cuisine.clone()),
            countries,
        });
    }
    Ok(places)
}

#[async_trait]
impl PlacesProvider for Overpass {
    fn name(&self) -> &'static str {
        "osm"
    }

    async fn search(&self, lat: f64, lng: f64, radius_m: u32) -> anyhow::Result<Vec<FoundPlace>> {
        let query = query(lat, lng, radius_m);
        let mut last_error = anyhow::anyhow!("no Overpass server configured");
        for url in &self.urls {
            match self.fetch(url, &query).await {
                Ok(places) => return Ok(places),
                Err(error) => {
                    tracing::warn!(url, error = format!("{error:#}"), "overpass server failed, trying the next");
                    last_error = error;
                }
            }
        }
        Err(last_error)
    }
}

pub struct Nominatim {
    http: reqwest::Client,
    url: String,
}

impl Nominatim {
    pub fn new(http: reqwest::Client, url: String) -> Self {
        Self { http, url }
    }
}

#[derive(Deserialize)]
struct NominatimPlace {
    display_name: String,
    lat: String,
    lon: String,
}

#[derive(Deserialize)]
struct NominatimReverse {
    name: Option<String>,
    #[serde(default)]
    address: HashMap<String, String>,
}

#[async_trait]
impl Geocoder for Nominatim {
    async fn search(&self, query: &str) -> anyhow::Result<Vec<GeoResult>> {
        let found: Vec<NominatimPlace> = self
            .http
            .get(format!("{}/search", self.url))
            .query(&[("q", query), ("format", "jsonv2"), ("limit", "5")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(found
            .into_iter()
            .filter_map(|p| {
                Some(GeoResult {
                    name: p.display_name,
                    lat: p.lat.parse().ok()?,
                    lng: p.lon.parse().ok()?,
                })
            })
            .collect())
    }

    async fn reverse(&self, lat: f64, lng: f64) -> anyhow::Result<Option<String>> {
        let found: NominatimReverse = self
            .http
            .get(format!("{}/reverse", self.url))
            .query(&[
                ("lat", lat.to_string()),
                ("lon", lng.to_string()),
                ("format", "jsonv2".into()),
                ("zoom", "10".into()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(["city", "town", "village", "municipality", "county"]
            .iter()
            .find_map(|k| found.address.get(*k).cloned())
            .or(found.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nodes_and_ways_and_skips_unmapped_cuisines() {
        let body = r#"{"elements":[
            {"type":"node","id":1,"lat":50.88,"lon":4.70,
             "tags":{"name":"Lalibela","cuisine":"ethiopian","addr:street":"Naamsestraat","addr:housenumber":"12","addr:city":"Leuven","website":"https://example.org"}},
            {"type":"way","id":2,"center":{"lat":50.87,"lon":4.71},
             "tags":{"name":"Saigon","cuisine":"vietnamese;thai"}},
            {"type":"node","id":3,"lat":50.0,"lon":4.0,"tags":{"name":"Burger Bar","cuisine":"burger"}},
            {"type":"relation","id":4,"tags":{"name":"Nowhere","cuisine":"greek"}}
        ]}"#;
        let places = parse(body).unwrap();
        assert_eq!(places.len(), 2);
        assert_eq!(places[0].id, "osm-node-1");
        assert_eq!(places[0].countries, vec!["ET"]);
        assert_eq!(places[0].address.as_deref(), Some("Naamsestraat 12, Leuven"));
        assert_eq!(places[1].id, "osm-way-2");
        assert_eq!(places[1].countries, vec!["VN", "TH"]);
        assert_eq!((places[1].lat, places[1].lng), (50.87, 4.71));
        assert_eq!(places[1].address, None);
    }
}
