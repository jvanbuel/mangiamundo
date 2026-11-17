//! OpenStreetMap: Overpass for restaurants, Nominatim for areas (cities and postal-code
//! boundaries). No API key needed.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{AreaDirectory, AreaInfo, AreaKind, Boundary, FoundPlace, Located, PlacesProvider};
use crate::geo::{Bbox, Shape};
use crate::{catalog, store};

/// The area kept as a local copy: Belgium plus a margin, so border towns such as
/// Lille, Maastricht and Aachen are covered too.
pub const IMPORT_BBOX: Bbox = Bbox { south: 49.4, west: 2.4, north: 51.6, east: 6.5 };
/// How often the local copy is refreshed, and how soon a failed import is retried.
const IMPORT_EVERY_SECS: i64 = 7 * 86_400;
const IMPORT_RETRY: Duration = Duration::from_secs(3600);
const IMPORTED_AT: &str = "osm_import_at";

pub struct Overpass {
    http: reqwest::Client,
    urls: Vec<String>,
}

impl Overpass {
    pub fn new(http: reqwest::Client, urls: Vec<String>) -> Self {
        Self { http, urls }
    }

    async fn fetch(&self, url: &str, query: &str, timeout: Duration) -> anyhow::Result<Vec<FoundPlace>> {
        let body = self
            .http
            .post(url)
            .timeout(timeout)
            .form(&[("data", query)])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        parse(&body)
    }

    /// Asks each server in turn until one answers.
    async fn fetch_any(&self, query: &str, timeout: Duration) -> anyhow::Result<Vec<FoundPlace>> {
        let mut last_error = anyhow::anyhow!("no Overpass server configured");
        for url in &self.urls {
            match self.fetch(url, query, timeout).await {
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

/// Restaurants in the area's bounding box; [`Overpass::search`] keeps the ones inside the
/// outline. A box query works for every area, also postal codes Overpass has no area for.
fn query(b: Bbox, timeout_secs: u64) -> String {
    format!(
        "[out:json][timeout:{timeout_secs}];\
         nwr[\"amenity\"~\"^(restaurant|fast_food|cafe|pub|bar)$\"][\"cuisine\"][\"name\"]\
         ({},{},{},{});\
         out center tags;",
        // Full precision: rounding could cut off a place just inside an edge.
        b.south, b.west, b.north, b.east
    )
}

/// OSM sometimes records a closure without removing the place.
fn is_closed(tags: &HashMap<String, String>, today: &str) -> bool {
    let hours_closed = tags
        .get("opening_hours")
        .is_some_and(|h| matches!(h.trim().to_ascii_lowercase().as_str(), "closed" | "off"));
    // end_date is "2024", "2024-05" or "2024-05-31": these compare correctly as text.
    let ended = tags.get("end_date").is_some_and(|d| d.as_str() <= today);
    hours_closed || ended
}

fn parse(body: &str) -> anyhow::Result<Vec<FoundPlace>> {
    let response: OverpassResponse = serde_json::from_str(body)?;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut places = Vec::new();
    for el in response.elements {
        let (Some(name), Some(cuisine)) = (el.tags.get("name"), el.tags.get("cuisine")) else {
            continue;
        };
        if is_closed(&el.tags, &today) {
            continue;
        }
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
            opening_hours: el.tags.get("opening_hours").cloned(),
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

    async fn search(&self, area: &Boundary) -> anyhow::Result<Vec<FoundPlace>> {
        let mut places = self.fetch_any(&query(area.shape.bbox(), 60), Duration::from_secs(75)).await?;
        places.retain(|p| area.shape.contains(p.lat, p.lng));
        Ok(places)
    }
}

/// Downloads every place in [`IMPORT_BBOX`] into the local copy. One large query a
/// week is kinder to the public servers than one per area, and a failure only delays
/// the refresh instead of breaking a user's request.
pub async fn import(db: &SqlitePool, overpass: &Overpass) -> anyhow::Result<usize> {
    let places = overpass.fetch_any(&query(IMPORT_BBOX, 300), Duration::from_secs(330)).await?;
    store::replace_imported(db, &places).await?;
    store::set_setting(db, IMPORTED_AT, &store::now().to_string()).await?;
    Ok(places.len())
}

/// Keeps the local copy fresh. Runs for the life of the server.
pub async fn run_imports(db: SqlitePool, overpass: Overpass) {
    loop {
        let last = store::setting(&db, IMPORTED_AT).await.ok().flatten().and_then(|v| v.parse::<i64>().ok());
        if last.is_none_or(|at| store::now() - at >= IMPORT_EVERY_SECS) {
            match import(&db, &overpass).await {
                Ok(count) => tracing::info!(count, "osm import finished"),
                Err(error) => tracing::warn!(error = format!("{error:#}"), "osm import failed, will retry"),
            }
        }
        tokio::time::sleep(IMPORT_RETRY).await;
    }
}

/// True when the whole box lies inside the import box.
fn covered(b: Bbox) -> bool {
    let i = IMPORT_BBOX;
    b.south >= i.south && b.north <= i.north && b.west >= i.west && b.east <= i.east
}

/// Answers from the local copy where it covers the area, and asks Overpass elsewhere.
pub struct WithImport {
    db: SqlitePool,
    live: Arc<dyn PlacesProvider>,
}

impl WithImport {
    pub fn new(db: SqlitePool, live: Arc<dyn PlacesProvider>) -> Self {
        Self { db, live }
    }
}

#[async_trait]
impl PlacesProvider for WithImport {
    /// Same name as the live provider: the ids match, so cached areas and visits carry over.
    fn name(&self) -> &'static str {
        self.live.name()
    }

    async fn search(&self, area: &Boundary) -> anyhow::Result<Vec<FoundPlace>> {
        let bbox = area.shape.bbox();
        if covered(bbox) && store::setting(&self.db, IMPORTED_AT).await?.is_some() {
            let mut places = store::imported_in(&self.db, bbox).await?;
            places.retain(|p| area.shape.contains(p.lat, p.lng));
            return Ok(places);
        }
        self.live.search(area).await
    }
}

/// Nominatim's usage policy allows one request per second.
const NOMINATIM_INTERVAL: Duration = Duration::from_secs(1);

/// What counts as a city: the municipality level, not districts or regions.
const CITY_TYPES: &[&str] = &["city", "town", "village", "municipality"];

pub struct Nominatim {
    http: reqwest::Client,
    url: String,
    /// When the last request went out; held during a request so they never overlap.
    last: tokio::sync::Mutex<Option<Instant>>,
}

impl Nominatim {
    pub fn new(http: reqwest::Client, url: String) -> Self {
        Self { http, url, last: tokio::sync::Mutex::new(None) }
    }

    async fn get(&self, path: &str, params: &[(&str, &str)]) -> anyhow::Result<Value> {
        let mut last = self.last.lock().await;
        if let Some(at) = *last {
            tokio::time::sleep(NOMINATIM_INTERVAL.saturating_sub(at.elapsed())).await;
        }
        *last = Some(Instant::now());
        let response = self
            .http
            .get(format!("{}/{path}", self.url))
            .query(&[("format", "jsonv2"), ("addressdetails", "1"), ("accept-language", "en")])
            .query(params)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(response)
    }

    /// Structured search for one kind of area around a point; the one containing the point wins.
    async fn containing(&self, params: &[(&str, &str)], lat: f64, lng: f64) -> anyhow::Result<Option<AreaInfo>> {
        let found = self.get("search", &[params, &[("polygon_geojson", "1"), ("polygon_threshold", "0.001")]].concat()).await?;
        let area = results(&found).find_map(|r| {
            let info = area_info(r)?;
            Shape::from_geojson(r.get("geojson")?)?.contains(lat, lng).then_some(info)
        });
        Ok(area)
    }
}

fn results(found: &Value) -> impl Iterator<Item = &Value> {
    found.as_array().into_iter().flatten()
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str().filter(|s| !s.is_empty())
}

/// A Nominatim result as an area, when it is a city or postal-code boundary.
fn area_info(r: &Value) -> Option<AreaInfo> {
    if text(r, "osm_type")? != "relation" || text(r, "category")? != "boundary" {
        return None;
    }
    let address = r.get("address").cloned().unwrap_or_default();
    let place = CITY_TYPES.iter().find_map(|k| text(&address, k));
    let (kind, name) = match (text(r, "type")?, text(r, "addresstype")) {
        ("postal_code", _) => {
            let code = text(&address, "postcode").or(text(r, "name"))?;
            (AreaKind::PostalCode, place.map_or(code.to_string(), |p| format!("{code} {p}")))
        }
        ("administrative", Some(t)) if CITY_TYPES.contains(&t) => (AreaKind::City, text(r, "name").or(place)?.to_string()),
        _ => return None,
    };
    let detail = [text(&address, "state").or(text(&address, "county")), text(&address, "country")]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
    Some(AreaInfo {
        id: format!("osm-relation-{}", r.get("osm_id")?.as_u64()?),
        name,
        kind,
        detail: (!detail.is_empty()).then_some(detail),
        lat: text(r, "lat")?.parse().ok()?,
        lng: text(r, "lon")?.parse().ok()?,
    })
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
        let Some(first) = results(&found).next() else { return Ok(Vec::new()) };
        let (Some(lat), Some(lng)) = (text(first, "lat").and_then(|v| v.parse().ok()), text(first, "lon").and_then(|v| v.parse().ok())) else {
            return Ok(Vec::new());
        };
        let located = self.locate(lat, lng).await?;
        Ok(located.city.into_iter().chain(located.postal_code).collect())
    }

    async fn locate(&self, lat: f64, lng: f64) -> anyhow::Result<Located> {
        // Reverse geocoding names the city and postal code, but returns the smallest
        // boundary (often a district), so look both up by name.
        let (lat_s, lng_s) = (lat.to_string(), lng.to_string());
        let here = self.get("reverse", &[("lat", &lat_s), ("lon", &lng_s), ("zoom", "18")]).await?;
        let address = here.get("address").cloned().unwrap_or_default();
        let Some(country) = text(&address, "country_code") else { return Ok(Located::default()) };
        let mut located = Located::default();
        if let Some(city) = CITY_TYPES.iter().find_map(|k| text(&address, k)) {
            // The state narrows common names (Springfield) so the right one is among the
            // results; without it as a fallback, in case the names don't line up.
            let by_name = [("city", city), ("countrycodes", country)];
            if let Some(state) = text(&address, "state") {
                located.city = self.containing(&[&by_name[..], &[("state", state)]].concat(), lat, lng).await?;
            }
            if located.city.is_none() {
                located.city = self.containing(&by_name, lat, lng).await?;
            }
        }
        if let Some(code) = text(&address, "postcode") {
            located.postal_code = self.containing(&[("postalcode", code), ("countrycodes", country)], lat, lng).await?;
        }
        Ok(located)
    }

    async fn boundary(&self, id: &str) -> anyhow::Result<Option<Boundary>> {
        let Some(relation) = id.strip_prefix("osm-relation-").filter(|n| n.bytes().all(|b| b.is_ascii_digit()) && !n.is_empty()) else {
            return Ok(None);
        };
        let ids = format!("R{relation}");
        let found = self
            .get("lookup", &[("osm_ids", &ids), ("polygon_geojson", "1"), ("polygon_threshold", "0.0003")])
            .await?;
        let boundary = results(&found)
            .next()
            .and_then(|r| Some(Boundary { info: area_info(r)?, shape: Shape::from_geojson(r.get("geojson")?)? }));
        Ok(boundary)
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

    #[test]
    fn reads_cities_and_postal_codes_and_skips_other_results() {
        let found = serde_json::json!([
            {"osm_type": "relation", "osm_id": 196311, "category": "boundary", "type": "administrative",
             "addresstype": "city", "name": "Leuven", "lat": "50.879", "lon": "4.701",
             "address": {"city": "Leuven", "state": "Flemish Brabant", "country": "Belgium"}},
            {"osm_type": "relation", "osm_id": 3366463, "category": "boundary", "type": "postal_code",
             "addresstype": "postcode", "name": "3000", "lat": "50.878", "lon": "4.702",
             "address": {"postcode": "3000", "city_district": "Leuven", "city": "Leuven", "country": "Belgium"}},
            {"osm_type": "relation", "osm_id": 3921680, "category": "boundary", "type": "administrative",
             "addresstype": "city_district", "name": "Leuven", "lat": "50.88", "lon": "4.70"},
            {"osm_type": "node", "osm_id": 1, "category": "place", "type": "city", "name": "Leuven", "lat": "50.88", "lon": "4.70"}
        ]);
        let areas: Vec<AreaInfo> = results(&found).filter_map(area_info).collect();
        assert_eq!(areas.len(), 2);
        assert_eq!(areas[0].id, "osm-relation-196311");
        assert_eq!(areas[0].kind, AreaKind::City);
        assert_eq!(areas[0].name, "Leuven");
        assert_eq!(areas[0].detail.as_deref(), Some("Flemish Brabant, Belgium"));
        assert_eq!(areas[1].kind, AreaKind::PostalCode);
        assert_eq!(areas[1].name, "3000 Leuven");
        assert_eq!(areas[1].detail.as_deref(), Some("Belgium"));
    }

    #[test]
    fn queries_the_bounding_box() {
        let b = Bbox { south: 50.82, west: 4.64, north: 50.94, east: 4.77 };
        assert!(query(b, 60).contains("(50.82,4.64,50.94,4.77)"));
        let precise = Bbox { south: 50.8242081, west: 4.000006, north: 50.9440737, east: 4.7704248 };
        assert!(query(precise, 60).contains("(50.8242081,4.000006,50.9440737,4.7704248)"), "no rounding");
    }

    /// Run by hand: `cargo test -- --ignored nominatim`.
    #[tokio::test]
    #[ignore = "talks to the public Nominatim server"]
    async fn nominatim_finds_leuven_and_its_postal_codes() {
        let http = reqwest::Client::builder().user_agent("mangiamundo-tests").build().unwrap();
        let nominatim = Nominatim::new(http, "https://nominatim.openstreetmap.org".into());
        let cities = nominatim.search("Leuven").await.unwrap();
        assert_eq!(cities[0].id, "osm-relation-196311", "{cities:?}");
        assert_eq!(cities[0].kind, AreaKind::City);
        let codes = nominatim.search("3000 Leuven").await.unwrap();
        assert!(codes.iter().any(|a| a.id == "osm-relation-3366463" && a.name == "3000 Leuven"), "{codes:?}");
        let here = nominatim.locate(50.8790, 4.7005).await.unwrap();
        assert_eq!(here.city.unwrap().id, "osm-relation-196311");
        assert_eq!(here.postal_code.unwrap().id, "osm-relation-3366463");
        let street = nominatim.search("Naamsestraat, Leuven").await.unwrap();
        assert_eq!(street.first().map(|a| a.id.as_str()), Some("osm-relation-196311"), "{street:?}");
        let boundary = nominatim.boundary("osm-relation-3366463").await.unwrap().unwrap();
        assert!(boundary.shape.contains(50.8790, 4.7005));
        assert!(!boundary.shape.contains(50.90, 4.73), "Wilsele is 3012, not 3000");
        assert!(nominatim.boundary("osm-relation-52411").await.unwrap().is_none(), "Belgium is not a city");
    }

    #[test]
    fn recorded_closures_are_skipped() {
        let tags = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let today = "2026-11-16";
        assert!(is_closed(&tags(&[("opening_hours", "closed")]), today));
        assert!(is_closed(&tags(&[("end_date", "2025")]), today));
        assert!(is_closed(&tags(&[("end_date", "2026-11-01")]), today));
        assert!(!is_closed(&tags(&[("end_date", "2027-01")]), today));
        assert!(!is_closed(&tags(&[("opening_hours", "Mo-Fr 11:00-22:00")]), today));
    }

    #[test]
    fn coverage_needs_the_whole_area_inside_the_box() {
        let b = |south, west, north, east| Bbox { south, west, north, east };
        assert!(covered(b(50.84, 4.65, 50.92, 4.76))); // Leuven
        assert!(covered(b(50.58, 2.97, 50.68, 3.13))); // Lille
        assert!(!covered(b(49.3, 5.0, 49.6, 5.3))); // crosses the southern edge
        assert!(!covered(b(48.81, 2.22, 48.90, 2.47))); // Paris
    }
}
