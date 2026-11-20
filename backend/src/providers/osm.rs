//! OpenStreetMap: Overpass for restaurants, Nominatim for areas (cities and postal-code
//! boundaries). No API key needed.

use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{AreaDirectory, AreaInfo, AreaKind, Boundary, FoundPlace, Located, Lookup, PlacesProvider};
use crate::geo::{Bbox, Shape};
use crate::{catalog, store};

/// The area kept as a local copy: Belgium plus a margin, so border towns such as
/// Lille, Maastricht and Aachen are covered too.
pub const IMPORT_BBOX: Bbox = Bbox { south: 49.4, west: 2.4, north: 51.6, east: 6.5 };
/// How often the local copy is refreshed, and how soon a failed import is retried.
const IMPORT_EVERY_SECS: i64 = 86_400;
const IMPORT_RETRY: Duration = Duration::from_secs(3600);
const IMPORTED_AT: &str = "osm_import_at";
/// For an area: how long a server gets before the next one is asked too.
const NEXT_SERVER_AFTER: Duration = Duration::from_secs(15);
/// At most this many area requests at once, however many servers are configured.
const MAX_RUNNING: usize = 3;
/// Expected time of an area query until one has been measured.
const FIRST_GUESS_SECS: f64 = 20.0;

pub struct Overpass {
    http: reqwest::Client,
    urls: Vec<String>,
    /// Running average of how long successful area queries took, in seconds.
    usual_secs: Mutex<f64>,
}

impl Overpass {
    pub fn new(http: reqwest::Client, urls: Vec<String>) -> Self {
        Self { http, urls, usual_secs: Mutex::new(FIRST_GUESS_SECS) }
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

    /// Asks the servers in order and returns the first answer. With `next_after`, the next
    /// server is also asked when the current one fails or hasn't answered in that time; the
    /// earlier requests keep running, as a slow server may still answer first. Without it, one
    /// server at a time, so a heavy query never runs twice.
    async fn fetch_any<'a>(
        &'a self,
        query: &'a str,
        timeout: Duration,
        next_after: Option<Duration>,
        on_next_server: &(dyn Fn() + Send + Sync),
    ) -> anyhow::Result<Vec<FoundPlace>> {
        let mut last_error = anyhow::anyhow!("no Overpass server configured");
        let mut urls = self.urls.iter();
        let ask = |url: &'a str| async move { (url, self.fetch(url, query, timeout).await) };
        let mut running = FuturesUnordered::new();
        let first = urls.next().ok_or_else(|| anyhow::anyhow!("no Overpass server configured"))?;
        running.push(ask(first));
        loop {
            // Another server only while fewer than MAX_RUNNING requests are out.
            let wait = next_after.filter(|_| urls.len() > 0 && running.len() < MAX_RUNNING);
            let next = tokio::select! {
                Some((url, result)) = running.next() => match result {
                    Ok(places) => return Ok(places),
                    Err(error) => {
                        tracing::warn!(url, error = format!("{error:#}"), "overpass server failed, trying the next");
                        last_error = error;
                        true
                    }
                },
                _ = tokio::time::sleep(wait.unwrap_or_default()), if wait.is_some() => {
                    tracing::info!("overpass server slow, asking the next one too");
                    true
                }
            };
            if next {
                match urls.next() {
                    Some(url) => {
                        on_next_server();
                        running.push(ask(url));
                    }
                    None if running.is_empty() => return Err(last_error),
                    None => {}
                }
            }
        }
    }

    /// Remembers how long a successful area query took.
    fn measured(&self, secs: f64) {
        let mut usual = self.usual_secs.lock().unwrap();
        *usual = 0.7 * *usual + 0.3 * secs;
    }
}

#[derive(Deserialize)]
struct OverpassResponse {
    elements: Vec<Element>,
    /// Set when the query failed on the server (a timeout, out of memory) even though the
    /// answer is a 200 with an empty `elements`.
    remark: Option<String>,
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

/// The kinds of place that count, as an Overpass tag filter.
const AMENITIES: &str = "[\"amenity\"~\"^(restaurant|fast_food|cafe|pub|bar)$\"]";

/// One element by its place id (`osm-node-123`), only while it is still a restaurant or the
/// like. `None` for ids that aren't OpenStreetMap elements.
fn element_query(id: &str) -> Option<String> {
    let (kind, n) = id.strip_prefix("osm-")?.split_once('-')?;
    let n: u64 = n.parse().ok()?;
    matches!(kind, "node" | "way" | "relation")
        .then(|| format!("[out:json][timeout:25];{kind}(id:{n}){AMENITIES};out center tags;"))
}

/// Restaurants in the area's bounding box; [`Overpass::search`] keeps the ones inside the
/// outline. A box query works for every area, also postal codes Overpass has no area for.
fn query(b: Bbox, timeout_secs: u64) -> String {
    format!(
        "[out:json][timeout:{timeout_secs}];\
         nwr{AMENITIES}[\"cuisine\"][\"name\"]\
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
    // A mapper marked it disused or abandoned without removing the live tags.
    let flagged = ["disused", "abandoned"].iter().any(|key| tags.get(*key).is_some_and(|v| v == "yes"));
    // `disused:amenity=restaurant` next to `amenity=restaurant`: closed, the old tag left
    // behind. A different value (`was:amenity=cafe` on a restaurant) only tells what the
    // place used to be, so it stays.
    let amenity = tags.get("amenity");
    let lifecycle = ["disused", "abandoned", "was", "demolished", "removed", "razed"]
        .iter()
        .any(|prefix| amenity.is_some() && tags.get(&format!("{prefix}:amenity")) == amenity);
    hours_closed || ended || flagged || lifecycle
}

fn parse(body: &str) -> anyhow::Result<Vec<FoundPlace>> {
    let response: OverpassResponse = serde_json::from_str(body)?;
    // An empty answer from a failed query must not read as "no restaurants": that would empty
    // an area, or hide a refreshed place. An error lets the next server try.
    if let Some(remark) = response.remark.as_deref().filter(|r| r.contains("error")) {
        anyhow::bail!("overpass query failed: {remark}");
    }
    Ok(places(response.elements))
}

/// The places among OSM elements: named, open, and serving a national cuisine.
fn places(elements: Vec<Element>) -> Vec<FoundPlace> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut places = Vec::new();
    for el in elements {
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
    places
}

/// The main OpenStreetMap API: answers for one element in a fraction of a second, with the
/// latest edit, where the public Overpass servers can queue a request for half a minute.
const OSM_API: &str = "https://api.openstreetmap.org/api/0.6";

/// A node, or a way with its nodes (`/way/{id}/full.json`), from the OSM API as a place.
/// The way gets the centre of its nodes' bounding box, as Overpass's `out center` gives it.
fn parse_api(kind: &str, id: u64, body: &str) -> anyhow::Result<Lookup> {
    #[derive(Deserialize)]
    struct Response {
        elements: Vec<Element>,
    }
    let mut elements = serde_json::from_str::<Response>(body)?.elements;
    let coords: Vec<(f64, f64)> =
        elements.iter().filter(|e| e.kind == "node").filter_map(|e| Some((e.lat?, e.lon?))).collect();
    let Some(mut el) = elements.drain(..).find(|e| e.kind == kind && e.id == id) else {
        return Ok(Lookup::Gone);
    };
    if kind == "way" && !coords.is_empty() {
        let (lats, lngs): (Vec<f64>, Vec<f64>) = coords.into_iter().unzip();
        let mid = |v: &[f64]| (v.iter().cloned().fold(f64::MAX, f64::min) + v.iter().cloned().fold(f64::MIN, f64::max)) / 2.0;
        el.center = Some(Center { lat: mid(&lats), lon: mid(&lngs) });
    }
    // Same filter as the Overpass element query.
    let amenity = el.tags.get("amenity").map(String::as_str);
    if !matches!(amenity, Some("restaurant" | "fast_food" | "cafe" | "pub" | "bar")) {
        return Ok(Lookup::Gone);
    }
    Ok(places(vec![el]).into_iter().next().map_or(Lookup::Gone, Lookup::Found))
}

impl Overpass {
    /// One node or way from the OSM API. `None` for relations, which Overpass reads instead.
    async fn read_osm_api(&self, id: &str) -> Option<anyhow::Result<Lookup>> {
        let (kind, n) = id.strip_prefix("osm-")?.split_once('-')?;
        let n: u64 = n.parse().ok()?;
        let url = match kind {
            "node" => format!("{OSM_API}/node/{n}.json"),
            "way" => format!("{OSM_API}/way/{n}/full.json"),
            _ => return None,
        };
        let answer = async {
            let response = self.http.get(&url).timeout(Duration::from_secs(10)).send().await?;
            // Deleted (410) or never there (404).
            if matches!(response.status().as_u16(), 404 | 410) {
                return Ok(Lookup::Gone);
            }
            parse_api(kind, n, &response.error_for_status()?.text().await?)
        };
        Some(answer.await)
    }
}

#[async_trait]
impl PlacesProvider for Overpass {
    fn name(&self) -> &'static str {
        "osm"
    }

    async fn search(&self, area: &Boundary, on_next_server: &(dyn Fn() + Send + Sync)) -> anyhow::Result<Vec<FoundPlace>> {
        let started = Instant::now();
        let query = query(area.shape.bbox(), 60);
        let mut places = self.fetch_any(&query, Duration::from_secs(75), Some(NEXT_SERVER_AFTER), on_next_server).await?;
        let secs = started.elapsed().as_secs_f64();
        tracing::info!(secs, places = places.len(), "overpass area query");
        self.measured(secs);
        places.retain(|p| area.shape.contains(p.lat, p.lng));
        Ok(places)
    }

    fn expected_secs(&self, _area: &Boundary) -> f64 {
        *self.usual_secs.lock().unwrap()
    }

    async fn place(&self, id: &str) -> anyhow::Result<Lookup> {
        let query = element_query(id).ok_or_else(|| anyhow::anyhow!("not an OpenStreetMap place: {id}"))?;
        match self.read_osm_api(id).await {
            Some(Ok(lookup)) => return Ok(lookup),
            Some(Err(error)) => tracing::warn!(id, error = format!("{error:#}"), "osm api failed, asking overpass"),
            None => {}
        }
        let places = self.fetch_any(&query, Duration::from_secs(30), Some(Duration::from_secs(8)), &|| ()).await?;
        Ok(places.into_iter().next().map_or(Lookup::Gone, Lookup::Found))
    }

    async fn nearby(&self, lat: f64, lng: f64, radius_m: f64) -> anyhow::Result<Vec<FoundPlace>> {
        let dlat = radius_m / 111_320.0;
        let dlng = dlat / lat.to_radians().cos().max(0.01);
        let b = Bbox { south: lat - dlat, west: lng - dlng, north: lat + dlat, east: lng + dlng };
        let mut places = self.fetch_any(&query(b, 25), Duration::from_secs(30), Some(Duration::from_secs(8)), &|| ()).await?;
        places.retain(|p| crate::geo::distance_m(lat, lng, p.lat, p.lng) <= radius_m);
        Ok(places)
    }
}

/// Downloads every place in [`IMPORT_BBOX`] into the local copy. One large query a
/// day is kinder to the public servers than one per area, and a failure only delays
/// the refresh instead of breaking a user's request.
pub async fn import(db: &SqlitePool, overpass: &Overpass) -> anyhow::Result<usize> {
    let places = overpass.fetch_any(&query(IMPORT_BBOX, 300), Duration::from_secs(330), None, &|| ()).await?;
    let matched = store::replace_imported(db, IMPORT_BBOX, &places).await?;
    tracing::info!(matched, "pending places matched to imported ones");
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

    async fn search(&self, area: &Boundary, on_next_server: &(dyn Fn() + Send + Sync)) -> anyhow::Result<Vec<FoundPlace>> {
        let bbox = area.shape.bbox();
        if covered(bbox) && store::setting(&self.db, IMPORTED_AT).await?.is_some() {
            let mut places = store::imported_in(&self.db, bbox).await?;
            places.retain(|p| area.shape.contains(p.lat, p.lng));
            return Ok(places);
        }
        self.live.search(area, on_next_server).await
    }

    fn expected_secs(&self, area: &Boundary) -> f64 {
        // ponytail: assumes the local copy exists; before the first import a covered area goes live.
        if covered(area.shape.bbox()) { 1.0 } else { self.live.expected_secs(area) }
    }

    /// Always live: the point is to see an edit before the next import.
    async fn place(&self, id: &str) -> anyhow::Result<Lookup> {
        self.live.place(id).await
    }

    async fn nearby(&self, lat: f64, lng: f64, radius_m: f64) -> anyhow::Result<Vec<FoundPlace>> {
        self.live.nearby(lat, lng, radius_m).await
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

    /// Nominatim's usage policy forbids search-as-you-type, so suggestions come from
    /// Photon, which is built for it and uses the same OSM ids.
    async fn suggest(&self, query: &str, near: Option<(f64, f64)>) -> anyhow::Result<Vec<AreaInfo>> {
        let mut request = self.http.get(PHOTON_URL).query(&[("q", query), ("limit", "15"), ("lang", "en")]);
        if let Some((lat, lng)) = near {
            request = request.query(&[("lat", lat), ("lon", lng)]);
        }
        let found: Value = request.send().await?.error_for_status()?.json().await?;
        Ok(photon_areas(&found))
    }
}

const PHOTON_URL: &str = "https://photon.komoot.io/api/";

/// The cities and postal codes among Photon's results.
fn photon_areas(found: &Value) -> Vec<AreaInfo> {
    let features = found.get("features").and_then(Value::as_array).into_iter().flatten();
    features
        .filter_map(|f| {
            let p = f.get("properties")?;
            if text(p, "osm_type")? != "R" {
                return None;
            }
            let name = text(p, "name")?;
            let (kind, name) = match (text(p, "type")?, text(p, "osm_value")) {
                ("city", _) => (AreaKind::City, name.to_string()),
                (_, Some("postcode")) => {
                    (AreaKind::PostalCode, text(p, "city").map_or(name.to_string(), |city| format!("{name} {city}")))
                }
                _ => return None,
            };
            let detail = [text(p, "state").or(text(p, "county")), text(p, "country")]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ");
            let point = f.get("geometry")?.get("coordinates")?.as_array()?;
            Some(AreaInfo {
                id: format!("osm-relation-{}", p.get("osm_id")?.as_u64()?),
                name,
                kind,
                detail: (!detail.is_empty()).then_some(detail),
                lat: point.get(1)?.as_f64()?,
                lng: point.first()?.as_f64()?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_failed_query_is_an_error_not_an_empty_answer() {
        let failed = r#"{"elements":[],"remark":"runtime error: Query timed out in \"query\" at line 1 after 26 seconds."}"#;
        assert!(parse(failed).is_err());
        assert!(parse(r#"{"elements":[]}"#).unwrap().is_empty(), "really nothing there");
    }

    #[test]
    fn element_queries_take_only_osm_element_ids() {
        let q = element_query("osm-way-1085544575").unwrap();
        assert!(q.contains("way(id:1085544575)[\"amenity\"~"), "{q}");
        assert!(element_query("osm-node-12").unwrap().contains("node(id:12)"));
        for bad in ["user-abc", "demo-leuven-1", "osm-area-3", "osm-node-1;out", "osm-node-"] {
            assert_eq!(element_query(bad), None, "{bad}");
        }
    }

    use super::*;

    #[test]
    fn osm_api_answers_become_places() {
        let node = r#"{"version":"0.6","elements":[{"type":"node","id":7,"lat":50.88,"lon":4.70,"tags":{"amenity":"restaurant","name":"Lalibela","cuisine":"ethiopian"}}]}"#;
        let Lookup::Found(place) = parse_api("node", 7, node).unwrap() else { panic!("not found") };
        assert_eq!((place.id.as_str(), place.lat, place.countries.clone()), ("osm-node-7", 50.88, vec!["ET"]));

        let way = r#"{"elements":[
            {"type":"node","id":1,"lat":50.0,"lon":4.0},{"type":"node","id":2,"lat":50.2,"lon":4.4},
            {"type":"way","id":9,"nodes":[1,2],"tags":{"amenity":"cafe","name":"Kissa","cuisine":"japanese"}}]}"#;
        let Lookup::Found(place) = parse_api("way", 9, way).unwrap() else { panic!("not found") };
        assert_eq!((place.id.as_str(), place.lat, place.lng), ("osm-way-9", 50.1, 4.2));

        // No longer a restaurant, or closed: gone, as the Overpass query would say.
        let shop = node.replace("restaurant", "shop");
        assert_eq!(parse_api("node", 7, &shop).unwrap(), Lookup::Gone);
        let closed = node.replace(r#""cuisine""#, r#""disused":"yes","cuisine""#);
        assert_eq!(parse_api("node", 7, &closed).unwrap(), Lookup::Gone);
    }

    /// A fake Overpass server: answers with `status` and one restaurant after `delay`.
    async fn fake_server(status: u16, delay: Duration) -> String {
        use axum::http::StatusCode;
        let app = axum::Router::new().route(
            "/api/interpreter",
            axum::routing::post(move || async move {
                tokio::time::sleep(delay).await;
                let body = r#"{"elements":[{"type":"node","id":1,"lat":50.88,"lon":4.70,"tags":{"name":"Lalibela","cuisine":"ethiopian"}}]}"#;
                (StatusCode::from_u16(status).unwrap(), body)
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/api/interpreter", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        url
    }

    #[tokio::test]
    async fn a_failing_or_slow_server_brings_in_the_next_one() {
        let failing = fake_server(502, Duration::ZERO).await;
        let slow = fake_server(200, Duration::from_secs(5)).await;
        let quick = fake_server(200, Duration::from_millis(10)).await;
        let overpass = Overpass::new(reqwest::Client::new(), vec![failing, slow, quick]);
        let asked = std::sync::atomic::AtomicUsize::new(0);
        let started = Instant::now();
        let places = overpass
            .fetch_any("q", Duration::from_secs(10), Some(Duration::from_millis(200)), &|| {
                asked.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            })
            .await
            .unwrap();
        assert_eq!(places[0].id, "osm-node-1");
        // The 502 brings in the slow server at once; 200 ms later the quick one answers first.
        assert_eq!(asked.into_inner(), 2);
        assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    }

    #[tokio::test]
    async fn without_a_deadline_servers_are_asked_one_at_a_time() {
        let slow = fake_server(200, Duration::from_millis(500)).await;
        let quick = fake_server(200, Duration::ZERO).await;
        let overpass = Overpass::new(reqwest::Client::new(), vec![slow, quick]);
        let asked = std::sync::atomic::AtomicUsize::new(0);
        overpass
            .fetch_any("q", Duration::from_secs(10), None, &|| {
                asked.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            })
            .await
            .unwrap();
        assert_eq!(asked.into_inner(), 0);
    }

    #[test]
    fn the_expected_time_follows_measured_queries() {
        let overpass = Overpass::new(reqwest::Client::new(), vec![]);
        for _ in 0..20 {
            overpass.measured(100.0);
        }
        assert!((99.0..=100.0).contains(&*overpass.usual_secs.lock().unwrap()));
    }

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
    fn reads_photon_cities_and_postal_codes() {
        let found = serde_json::json!({"features": [
            {"geometry": {"coordinates": [4.70, 50.88]}, "properties":
             {"osm_type": "R", "osm_id": 3366463, "osm_value": "postcode", "type": "other", "name": "3000",
              "city": "Leuven", "state": "Flemish Brabant", "country": "Belgium"}},
            {"geometry": {"coordinates": [4.70, 50.88]}, "properties":
             {"osm_type": "N", "osm_id": 1, "osm_value": "sports_centre", "type": "house", "name": "Laser 3000"}},
            {"geometry": {"coordinates": [4.70, 50.88]}, "properties":
             {"osm_type": "R", "osm_id": 196311, "osm_value": "city", "type": "city", "name": "Leuven",
              "state": "Flemish Brabant", "country": "Belgium"}},
            {"geometry": {"coordinates": [4.69, 50.88]}, "properties":
             {"osm_type": "R", "osm_id": 3921680, "osm_value": "administrative", "type": "district", "name": "Leuven"}}
        ]});
        let areas = photon_areas(&found);
        assert_eq!(areas.len(), 2);
        assert_eq!((areas[0].id.as_str(), areas[0].name.as_str(), areas[0].kind), ("osm-relation-3366463", "3000 Leuven", AreaKind::PostalCode));
        assert_eq!((areas[1].id.as_str(), areas[1].name.as_str(), areas[1].kind), ("osm-relation-196311", "Leuven", AreaKind::City));
        assert_eq!(areas[1].detail.as_deref(), Some("Flemish Brabant, Belgium"));
        assert_eq!((areas[1].lat, areas[1].lng), (50.88, 4.70));
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

    /// Run by hand: `cargo test -- --ignored osm_api_reads`.
    #[tokio::test]
    #[ignore = "talks to the public OpenStreetMap API"]
    async fn osm_api_reads_a_place_quickly() {
        let http = reqwest::Client::builder().user_agent("mangiamundo-tests").build().unwrap();
        // Unreachable Overpass: the answer has to come from the OSM API.
        let overpass = Overpass::new(http, vec!["http://127.0.0.1:9/api/interpreter".into()]);
        let started = Instant::now();
        // An Italian restaurant in Leuven.
        let lookup = overpass.place("osm-node-365439479").await.unwrap();
        assert!(matches!(lookup, Lookup::Found(_)), "{lookup:?}");
        assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
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
        // Disused or abandoned, with the live tags still on.
        assert!(is_closed(&tags(&[("amenity", "fast_food"), ("disused", "yes")]), today));
        assert!(is_closed(&tags(&[("amenity", "fast_food"), ("abandoned", "yes")]), today));
        assert!(!is_closed(&tags(&[("amenity", "fast_food"), ("disused", "no")]), today));
        for prefix in ["disused", "abandoned", "was"] {
            let key = format!("{prefix}:amenity");
            assert!(is_closed(&tags(&[("amenity", "fast_food"), (&key, "fast_food")]), today), "{key}");
            // What it used to be, before it became what it is.
            assert!(!is_closed(&tags(&[("amenity", "restaurant"), (&key, "cafe")]), today), "{key}");
        }
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
