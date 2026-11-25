//! The main OpenStreetMap API, for reading one place quickly.

use std::time::Duration;

use serde::Deserialize;

use super::elements::{places, Center, Element};
use super::Overpass;
use crate::providers::Lookup;

/// The main OpenStreetMap API: answers for one element in a fraction of a second, with the
/// latest edit, where the public Overpass servers can queue a request for half a minute.
const OSM_API: &str = "https://api.openstreetmap.org/api/0.6";

/// A node, or a way with its nodes (`/way/{id}/full.json`), from the OSM API as a place.
/// The way gets the centre of its nodes' bounding box, as Overpass's `out center` gives it.
pub(super) fn parse_api(kind: &str, id: u64, body: &str) -> anyhow::Result<Lookup> {
    #[derive(Deserialize)]
    struct Response {
        elements: Vec<Element>,
    }
    let mut elements = serde_json::from_str::<Response>(body)?.elements;
    let coords: Vec<(f64, f64)> = elements
        .iter()
        .filter(|e| e.kind == "node")
        .filter_map(|e| Some((e.lat?, e.lon?)))
        .collect();
    let Some(mut el) = elements.drain(..).find(|e| e.kind == kind && e.id == id) else {
        return Ok(Lookup::Gone);
    };
    if kind == "way" && !coords.is_empty() {
        let (lats, lngs): (Vec<f64>, Vec<f64>) = coords.into_iter().unzip();
        let min = |v: &[f64]| v.iter().copied().fold(f64::MAX, f64::min);
        let max = |v: &[f64]| v.iter().copied().fold(f64::MIN, f64::max);
        // No building is half a world wide: it crosses the antimeridian, where the plain
        // midpoint lands on the other side. Rare enough to leave to Overpass.
        if max(&lngs) - min(&lngs) > 180.0 {
            anyhow::bail!("way {id} crosses the antimeridian");
        }
        el.center = Some(Center {
            lat: f64::midpoint(min(&lats), max(&lats)),
            lon: f64::midpoint(min(&lngs), max(&lngs)),
        });
    }
    // Same filter as the Overpass element query.
    let amenity = el.tags.get("amenity").map(String::as_str);
    if !matches!(
        amenity,
        Some("restaurant" | "fast_food" | "cafe" | "pub" | "bar")
    ) {
        return Ok(Lookup::Gone);
    }
    Ok(places(vec![el])
        .into_iter()
        .next()
        .map_or(Lookup::Gone, Lookup::Found))
}

impl Overpass {
    /// One node or way from the OSM API. `None` for relations, which Overpass reads instead.
    pub(super) async fn read_osm_api(&self, id: &str) -> Option<anyhow::Result<Lookup>> {
        let (kind, n) = id.strip_prefix("osm-")?.split_once('-')?;
        let n: u64 = n.parse().ok()?;
        let url = match kind {
            "node" => format!("{OSM_API}/node/{n}.json"),
            "way" => format!("{OSM_API}/way/{n}/full.json"),
            _ => return None,
        };
        let answer = async {
            let response = self
                .http
                .get(&url)
                .timeout(Duration::from_secs(10))
                .send()
                .await?;
            // Deleted (410) or never there (404).
            if matches!(response.status().as_u16(), 404 | 410) {
                return Ok(Lookup::Gone);
            }
            parse_api(kind, n, &response.error_for_status()?.text().await?)
        };
        Some(answer.await)
    }
}
