//! Reading Overpass elements as places.

use std::collections::HashMap;

use serde::Deserialize;

use crate::providers::FoundPlace;
use crate::{catalog, store};

#[derive(Deserialize)]
struct OverpassResponse {
    elements: Vec<Element>,
    /// Set when the query failed on the server (a timeout, out of memory) even though the
    /// answer is a 200 with an empty `elements`.
    remark: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct Element {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) id: u64,
    pub(super) lat: Option<f64>,
    pub(super) lon: Option<f64>,
    pub(super) center: Option<Center>,
    #[serde(default)]
    pub(super) tags: HashMap<String, String>,
}

#[derive(Deserialize)]
pub(super) struct Center {
    pub(super) lat: f64,
    pub(super) lon: f64,
}

/// OSM sometimes records a closure without removing the place.
pub(super) fn is_closed(tags: &HashMap<String, String>, today: &str) -> bool {
    let hours_closed = tags
        .get("opening_hours")
        .is_some_and(|h| matches!(h.trim().to_ascii_lowercase().as_str(), "closed" | "off"));
    // end_date is "2024", "2024-05" or "2024-05-31": these compare correctly as text.
    let ended = tags.get("end_date").is_some_and(|d| d.as_str() <= today);
    // A mapper marked it disused or abandoned without removing the live tags.
    let flagged = ["disused", "abandoned"]
        .iter()
        .any(|key| tags.get(*key).is_some_and(|v| v == "yes"));
    // `disused:amenity=restaurant` next to `amenity=restaurant`: closed, the old tag left
    // behind. A different value (`was:amenity=cafe` on a restaurant) only tells what the
    // place used to be, so it stays.
    let amenity = tags.get("amenity");
    let lifecycle = [
        "disused",
        "abandoned",
        "was",
        "demolished",
        "removed",
        "razed",
    ]
    .iter()
    .any(|prefix| amenity.is_some() && tags.get(&format!("{prefix}:amenity")) == amenity);
    hours_closed || ended || flagged || lifecycle
}

pub(super) fn elements(body: &str) -> anyhow::Result<Vec<Element>> {
    let response: OverpassResponse = serde_json::from_str(body)?;
    // An empty answer from a failed query must not read as "no restaurants": that would empty
    // an area, or hide a refreshed place. An error lets the next server try.
    if let Some(remark) = response.remark.as_deref().filter(|r| r.contains("error")) {
        anyhow::bail!("overpass query failed: {remark}");
    }
    Ok(response.elements)
}

#[cfg(test)]
pub(super) fn parse(body: &str) -> anyhow::Result<Vec<FoundPlace>> {
    elements(body).map(places)
}

fn position(el: &Element) -> Option<(f64, f64)> {
    match (el.lat, el.lon, &el.center) {
        (Some(lat), Some(lon), _) => Some((lat, lon)),
        (_, _, Some(c)) => Some((c.lat, c.lon)),
        _ => None,
    }
}

/// "Street 12, City" from the `addr:*` tags, `None` when there are none.
fn address(tags: &HashMap<String, String>) -> Option<String> {
    let street = [tags.get("addr:street"), tags.get("addr:housenumber")]
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    let address = [
        Some(street.as_str()),
        tags.get("addr:city").map(String::as_str),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(", ");
    (!address.is_empty()).then_some(address)
}

/// The named, open places among OSM elements that have no `cuisine` tag at all.
pub(super) fn unknown(elements: &[Element]) -> Vec<store::UnknownPlace> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    elements
        .iter()
        .filter(|el| !el.tags.contains_key("cuisine") && !is_closed(&el.tags, &today))
        .filter_map(|el| {
            let (lat, lng) = position(el)?;
            Some(store::UnknownPlace {
                id: format!("osm-{}-{}", el.kind, el.id),
                name: el.tags.get("name")?.clone(),
                lat,
                lng,
                address: address(&el.tags),
                distance_m: 0,
            })
        })
        .collect()
}

/// The places among OSM elements: named, open, and serving a national cuisine.
pub(super) fn places(elements: Vec<Element>) -> Vec<FoundPlace> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut places = Vec::new();
    let mut dropped: HashMap<String, usize> = HashMap::new();
    for el in elements {
        let (Some(name), Some(cuisine)) = (el.tags.get("name"), el.tags.get("cuisine")) else {
            continue;
        };
        if is_closed(&el.tags, &today) {
            continue;
        }
        let countries = catalog::countries_for_cuisine_tag(cuisine);
        if countries.is_empty() {
            // Only tags that cost us the whole place: the extra `pizza` in `italian;pizza`
            // loses nothing and would crowd out the ones that do.
            for tag in cuisine.split([';', ',']) {
                *dropped.entry(tag.trim().to_lowercase()).or_default() += 1;
            }
            continue;
        }
        let Some((lat, lng)) = position(&el) else {
            continue;
        };
        places.push(FoundPlace {
            id: format!("osm-{}-{}", el.kind, el.id),
            name: name.clone(),
            lat,
            lng,
            address: address(&el.tags),
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
    if !dropped.is_empty() {
        // Which tags the catalog misses, most common first: the list to extend it from.
        let mut top: Vec<_> = dropped.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let top = top
            .iter()
            .take(30)
            .map(|(tag, n)| format!("{tag}:{n}"))
            .collect::<Vec<_>>()
            .join(", ");
        tracing::info!(top, "cuisine tags without a country");
    }
    places
}
