//! Static reference data: countries, continents and the cuisine → country mapping.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

mod cuisines;

pub use cuisines::{Cuisine, CUISINES};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Country {
    /// ISO 3166-1 alpha-2.
    pub code: String,
    /// ISO 3166-1 numeric, zero padded. Matches the ids in the world-atlas map data.
    #[schema(required = true)]
    pub numeric: Option<String>,
    pub name: String,
    pub continent: String,
    pub flag: String,
    pub lat: f64,
    pub lng: f64,
    /// The OpenStreetMap `cuisine` value for its food, like `sri_lankan`; `null` when the
    /// catalog has none. Only countries with one can be picked for a new place.
    #[serde(default)]
    #[schema(required = true)]
    pub cuisine: Option<&'static str>,
}

pub const CONTINENTS: &[(&str, &str)] = &[
    ("AF", "Africa"),
    ("AS", "Asia"),
    ("EU", "Europe"),
    ("NA", "North America"),
    ("OC", "Oceania"),
    ("SA", "South America"),
];

pub fn countries() -> &'static [Country] {
    static DATA: OnceLock<Vec<Country>> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut all: Vec<Country> = serde_json::from_str(include_str!("../../data/countries.json"))
            .expect("data/countries.json is valid");
        for c in &mut all {
            c.cuisine = cuisine_tag(&c.code);
        }
        all
    })
}

pub fn country(code: &str) -> Option<&'static Country> {
    static INDEX: OnceLock<HashMap<&'static str, &'static Country>> = OnceLock::new();
    INDEX
        .get_or_init(|| countries().iter().map(|c| (c.code.as_str(), c)).collect())
        .get(code)
        .copied()
}

/// The canonical OpenStreetMap `cuisine` value for a country, like `sri_lankan`.
pub fn cuisine_tag(country: &str) -> Option<&'static str> {
    Some(CUISINES.iter().find(|c| c.country == country)?.osm[0])
}

/// The cuisine's adjective, like "Lebanese" or "Sri Lankan", when the catalog has one.
pub fn cuisine_adjective(country: &str) -> Option<String> {
    let tag = cuisine_tag(country)?;
    Some(
        tag.split('_')
            .map(|word| {
                let mut chars = word.chars();
                chars
                    .next()
                    .map_or_default(|first| first.to_uppercase().chain(chars).collect::<String>())
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Maps a raw `cuisine` tag value ("italian;pizza", "Thai", "sri lankan") to country codes.
pub fn countries_for_cuisine_tag(raw: &str) -> Vec<&'static str> {
    static INDEX: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        CUISINES
            .iter()
            .flat_map(|cu| cu.osm.iter().map(move |tag| (*tag, cu.country)))
            .collect()
    });

    let mut out: Vec<&'static str> = Vec::new();
    for part in raw.split([';', ',']) {
        let key = part.trim().to_lowercase().replace(' ', "_");
        if let Some(code) = index.get(key.as_str()) {
            if !out.contains(code) {
                out.push(code);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
