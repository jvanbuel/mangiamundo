//! Static reference data: countries, continents and the cuisine → country mapping.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Country {
    /// ISO 3166-1 alpha-2.
    pub code: String,
    /// ISO 3166-1 numeric, zero padded. Matches the ids in the world-atlas map data.
    pub numeric: Option<String>,
    pub name: String,
    pub continent: String,
    pub flag: String,
    pub lat: f64,
    pub lng: f64,
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
        serde_json::from_str(include_str!("../data/countries.json"))
            .expect("data/countries.json is valid")
    })
}

pub fn country(code: &str) -> Option<&'static Country> {
    static INDEX: OnceLock<HashMap<&'static str, &'static Country>> = OnceLock::new();
    INDEX
        .get_or_init(|| countries().iter().map(|c| (c.code.as_str(), c)).collect())
        .get(code)
        .copied()
}

/// One national cuisine and how OpenStreetMap refers to it.
pub struct Cuisine {
    pub country: &'static str,
    /// Values of the OpenStreetMap `cuisine=*` tag. The first one is the canonical demonym.
    pub osm: &'static [&'static str],
}

const fn c(country: &'static str, osm: &'static [&'static str]) -> Cuisine {
    Cuisine { country, osm }
}

/// Only cuisines that point at a single country are listed. Regional labels such as
/// "asian", "african" or "mediterranean" are deliberately left out, and so are dishes
/// that have gone global (pizza, kebab, burger): they say little about a country.
pub const CUISINES: &[Cuisine] = &[
    // Europe
    c("AL", &["albanian"]),
    c("AT", &["austrian"]),
    c("BA", &["bosnian"]),
    c("BE", &["belgian"]),
    c("BG", &["bulgarian"]),
    c("BY", &["belarusian"]),
    c("CH", &["swiss"]),
    c("CY", &["cypriot"]),
    c("CZ", &["czech"]),
    c("DE", &["german", "bavarian"]),
    c("DK", &["danish"]),
    c("EE", &["estonian"]),
    c("ES", &["spanish", "tapas", "basque", "catalan"]),
    c("FI", &["finnish"]),
    c("FR", &["french", "crepe"]),
    c("GB", &["british", "english", "scottish", "fish_and_chips"]),
    c("GE", &["georgian"]),
    c("GR", &["greek"]),
    c("HR", &["croatian"]),
    c("HU", &["hungarian"]),
    c("IE", &["irish"]),
    c("IS", &["icelandic"]),
    c("IT", &["italian", "italian_pizza", "pasta"]),
    c("LT", &["lithuanian"]),
    c("LV", &["latvian"]),
    c("MT", &["maltese"]),
    c("NL", &["dutch"]),
    c("NO", &["norwegian"]),
    c("PL", &["polish"]),
    c("PT", &["portuguese"]),
    c("RO", &["romanian"]),
    c("RS", &["serbian"]),
    c("RU", &["russian"]),
    c("SE", &["swedish"]),
    c("SI", &["slovenian"]),
    c("SK", &["slovak"]),
    c("UA", &["ukrainian"]),
    // Asia
    c("AF", &["afghan"]),
    c("AM", &["armenian"]),
    c("AZ", &["azerbaijani"]),
    c("BD", &["bangladeshi"]),
    c("CN", &["chinese", "sichuan", "cantonese", "dim_sum", "hotpot"]),
    c("ID", &["indonesian"]),
    c("IL", &["israeli"]),
    c("IN", &["indian", "south_indian", "punjabi"]),
    c("IQ", &["iraqi"]),
    c("IR", &["persian", "iranian"]),
    c("JO", &["jordanian"]),
    c("JP", &["japanese", "sushi", "ramen", "udon", "soba", "yakitori", "okonomiyaki"]),
    c("KH", &["cambodian"]),
    c("KR", &["korean"]),
    c("KZ", &["kazakh"]),
    c("LA", &["lao", "laotian"]),
    c("LB", &["lebanese"]),
    c("LK", &["sri_lankan"]),
    c("MM", &["burmese"]),
    c("MN", &["mongolian"]),
    c("MY", &["malaysian", "malay"]),
    c("NP", &["nepalese", "nepali"]),
    c("PH", &["filipino"]),
    c("PK", &["pakistani"]),
    c("PS", &["palestinian"]),
    c("SG", &["singaporean"]),
    c("SY", &["syrian"]),
    c("TH", &["thai"]),
    c("TR", &["turkish"]),
    c("TW", &["taiwanese", "bubble_tea"]),
    c("UZ", &["uzbek"]),
    c("VN", &["vietnamese", "pho"]),
    c("YE", &["yemeni"]),
    // Africa
    c("CD", &["congolese"]),
    c("CI", &["ivorian"]),
    c("CM", &["cameroonian"]),
    c("DZ", &["algerian"]),
    c("EG", &["egyptian"]),
    c("ER", &["eritrean"]),
    c("ET", &["ethiopian"]),
    c("GH", &["ghanaian"]),
    c("KE", &["kenyan"]),
    c("MA", &["moroccan"]),
    c("NG", &["nigerian"]),
    c("SD", &["sudanese"]),
    c("SN", &["senegalese"]),
    c("SO", &["somali"]),
    c("TN", &["tunisian"]),
    c("ZA", &["south_african"]),
    // North America
    c("CA", &["canadian"]),
    c("CU", &["cuban"]),
    c("DO", &["dominican"]),
    c("HT", &["haitian"]),
    c("JM", &["jamaican"]),
    c("MX", &["mexican", "tex-mex", "tacos"]),
    c("SV", &["salvadoran"]),
    c("US", &["american", "cajun", "hawaiian", "diner"]),
    // South America
    c("AR", &["argentinian", "argentine"]),
    c("BO", &["bolivian"]),
    c("BR", &["brazilian"]),
    c("CL", &["chilean"]),
    c("CO", &["colombian"]),
    c("EC", &["ecuadorian"]),
    c("PE", &["peruvian"]),
    c("SR", &["surinamese"]),
    c("VE", &["venezuelan"]),
    // Oceania
    c("AU", &["australian"]),
    c("NZ", &["new_zealand"]),
];

/// The cuisine's adjective, like "Lebanese" or "Sri Lankan", when the catalog has one.
pub fn cuisine_adjective(country: &str) -> Option<String> {
    let tag = CUISINES.iter().find(|c| c.country == country)?.osm[0];
    Some(
        tag.split('_')
            .map(|word| {
                let mut chars = word.chars();
                chars.next().map(|first| first.to_uppercase().chain(chars).collect::<String>()).unwrap_or_default()
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
mod tests {
    use super::*;

    #[test]
    fn every_cuisine_points_at_a_known_country() {
        for cu in CUISINES {
            assert!(country(cu.country).is_some(), "unknown country {}", cu.country);
            assert!(!cu.osm.is_empty());
        }
    }

    #[test]
    fn each_country_is_listed_once_and_tags_are_unique() {
        let mut seen_country = std::collections::HashSet::new();
        let mut seen_tag = std::collections::HashSet::new();
        for cu in CUISINES {
            assert!(seen_country.insert(cu.country), "duplicate {}", cu.country);
            for tag in cu.osm {
                assert!(seen_tag.insert(*tag), "duplicate tag {tag}");
            }
        }
    }

    #[test]
    fn every_country_has_a_known_continent() {
        for co in countries() {
            assert!(CONTINENTS.iter().any(|(code, _)| *code == co.continent), "{}", co.code);
        }
    }

    #[test]
    fn names_cuisines() {
        assert_eq!(cuisine_adjective("LK").as_deref(), Some("Sri Lankan"));
        assert_eq!(cuisine_adjective("LB").as_deref(), Some("Lebanese"));
        assert_eq!(cuisine_adjective("AQ"), None);
    }

    #[test]
    fn parses_multi_valued_tags() {
        assert_eq!(countries_for_cuisine_tag("italian;pizza"), vec!["IT"]);
        assert_eq!(countries_for_cuisine_tag("Thai; vietnamese"), vec!["TH", "VN"]);
        assert_eq!(countries_for_cuisine_tag("Sri Lankan"), vec!["LK"]);
        assert!(countries_for_cuisine_tag("burger;regional").is_empty());
    }
}
