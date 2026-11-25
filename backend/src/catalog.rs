//! Static reference data: countries, continents and the cuisine → country mapping.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

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
        let mut all: Vec<Country> = serde_json::from_str(include_str!("../data/countries.json"))
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
    c("AT", &["austrian", "heuriger", "buschenschank"]),
    c("BA", &["bosnian"]),
    c(
        "BE",
        &["belgian", "friture", "friterie", "friteries", "frites"],
    ),
    c("BG", &["bulgarian", "banitsa"]),
    c("BY", &["belarusian"]),
    c("CH", &["swiss"]),
    c("CY", &["cypriot"]),
    c("CZ", &["czech"]),
    c("DE", &["german", "bavarian", "swabian", "currywurst"]),
    c("DK", &["danish", "smørrebrød"]),
    c("EE", &["estonian"]),
    c(
        "ES",
        &[
            "spanish",
            "tapas",
            "basque",
            "catalan",
            "galician",
            "paella",
            "basque_ciderhouse",
        ],
    ),
    c("FI", &["finnish"]),
    c(
        "FR",
        &[
            "french",
            "crepe",
            "french_tacos",
            "flammekueche",
            "corsican",
        ],
    ),
    c("GB", &["british", "english", "scottish", "fish_and_chips"]),
    c("GE", &["georgian"]),
    c("GR", &["greek", "gyros", "souvlaki"]),
    c("HR", &["croatian"]),
    c("HU", &["hungarian", "langos", "lángos"]),
    c("IE", &["irish"]),
    c("IS", &["icelandic"]),
    c(
        "IT",
        &["italian", "italian_pizza", "pasta", "piadina", "sicilian"],
    ),
    c("LT", &["lithuanian"]),
    c("LU", &["luxembourgish"]),
    c("LV", &["latvian"]),
    c("MD", &["moldovan"]),
    c("ME", &["montenegrin"]),
    c("MK", &["macedonian"]),
    c("MT", &["maltese"]),
    c("NL", &["dutch"]),
    c("NO", &["norwegian"]),
    c("PL", &["polish", "pierogi"]),
    c("PT", &["portuguese"]),
    c("RO", &["romanian"]),
    c("RS", &["serbian"]),
    c("RU", &["russian"]),
    c("SE", &["swedish"]),
    c("SI", &["slovenian"]),
    c("SK", &["slovak"]),
    c("UA", &["ukrainian"]),
    c("XK", &["kosovar"]),
    // Asia
    c("AE", &["emirati"]),
    c("AF", &["afghan"]),
    c("AM", &["armenian"]),
    c("AZ", &["azerbaijani"]),
    c("BD", &["bangladeshi"]),
    c(
        "CN",
        &[
            "chinese",
            "sichuan",
            "cantonese",
            "dim_sum",
            "hotpot",
            "hot_pot",
            "jiaozi",
            "baozi",
        ],
    ),
    c("ID", &["indonesian", "padang"]),
    c("IL", &["israeli"]),
    c("IN", &["indian", "south_indian", "punjabi"]),
    c("IQ", &["iraqi"]),
    c("IR", &["persian", "iranian"]),
    c("JO", &["jordanian"]),
    c(
        "JP",
        &[
            "japanese",
            "sushi",
            "ramen",
            "udon",
            "soba",
            "yakitori",
            "okonomiyaki",
            "tempura",
            "yakiniku",
            "teppanyaki",
            "takoyaki",
            "izakaya",
            "tonkatsu",
            "gyoza",
            "unagi",
            "bento",
        ],
    ),
    c("KH", &["cambodian"]),
    c("KG", &["kyrgyz"]),
    c("KR", &["korean", "korean_bbq", "korean_food", "gimbap"]),
    c("KZ", &["kazakh"]),
    c("LA", &["lao", "laotian"]),
    c("KW", &["kuwaiti"]),
    c("LB", &["lebanese"]),
    c("LK", &["sri_lankan"]),
    c("MM", &["burmese", "myanmar"]),
    c("MN", &["mongolian"]),
    c("MY", &["malaysian", "malay", "mamak"]),
    c("NP", &["nepalese", "nepali"]),
    c("OM", &["omani"]),
    c("PH", &["filipino"]),
    c("PK", &["pakistani"]),
    c("PS", &["palestinian"]),
    c("SA", &["saudi"]),
    c("SG", &["singaporean"]),
    c("SY", &["syrian"]),
    c("TH", &["thai"]),
    c("TJ", &["tajik"]),
    c("TM", &["turkmen"]),
    c("TR", &["turkish"]),
    c("TW", &["taiwanese", "bubble_tea", "beef_noodle"]),
    c("UZ", &["uzbek"]),
    c("VN", &["vietnamese", "pho"]),
    c("YE", &["yemeni"]),
    // Africa
    c("AO", &["angolan"]),
    c("CD", &["congolese"]),
    c("CI", &["ivorian"]),
    c("CM", &["cameroonian"]),
    c("CV", &["cape_verdean"]),
    c("DZ", &["algerian"]),
    c("EG", &["egyptian"]),
    c("ER", &["eritrean"]),
    c("ET", &["ethiopian"]),
    c("GH", &["ghanaian"]),
    c("KE", &["kenyan"]),
    c("MA", &["moroccan"]),
    c("MG", &["malagasy"]),
    c("MU", &["mauritian"]),
    c("NG", &["nigerian"]),
    c("RW", &["rwandan"]),
    c("SD", &["sudanese"]),
    c("SN", &["senegalese"]),
    c("SO", &["somali"]),
    c("TN", &["tunisian"]),
    c("TZ", &["tanzanian"]),
    c("UG", &["ugandan"]),
    c("ZA", &["south_african"]),
    c("ZW", &["zimbabwean"]),
    // North America
    c("CA", &["canadian", "poutine"]),
    c("CR", &["costa_rican"]),
    c("CU", &["cuban"]),
    c("DO", &["dominican"]),
    c("GT", &["guatemalan"]),
    c("HN", &["honduran"]),
    c("HT", &["haitian"]),
    c("JM", &["jamaican"]),
    c("MX", &["mexican", "tex-mex", "tacos", "burrito"]),
    c("NI", &["nicaraguan"]),
    c("PA", &["panamanian"]),
    c("PR", &["puerto_rican"]),
    c("SV", &["salvadoran"]),
    c("TT", &["trinidadian"]),
    c(
        "US",
        &[
            "american",
            "cajun",
            "hawaiian",
            "diner",
            "poke",
            "soul_food",
            "new_american",
        ],
    ),
    // South America
    c("AR", &["argentinian", "argentine"]),
    c("BO", &["bolivian"]),
    c("BR", &["brazilian"]),
    c("CL", &["chilean"]),
    c("CO", &["colombian"]),
    c("EC", &["ecuadorian"]),
    c("PE", &["peruvian", "ceviche", "chifa"]),
    c("PY", &["paraguayan"]),
    c("SR", &["surinamese"]),
    c("UY", &["uruguayan"]),
    c("VE", &["venezuelan"]),
    // Oceania
    c("AU", &["australian"]),
    c("NZ", &["new_zealand"]),
];

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
mod tests {
    use super::*;

    #[test]
    fn every_cuisine_points_at_a_known_country() {
        for cu in CUISINES {
            assert!(
                country(cu.country).is_some(),
                "unknown country {}",
                cu.country
            );
            assert_ne!(cu.osm, [] as [&str; 0]);
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
            assert!(
                CONTINENTS.iter().any(|(code, _)| *code == co.continent),
                "{}",
                co.code
            );
        }
    }

    #[test]
    fn names_cuisines() {
        assert_eq!(cuisine_adjective("LK").as_deref(), Some("Sri Lankan"));
        assert_eq!(cuisine_adjective("LB").as_deref(), Some("Lebanese"));
        assert_eq!(cuisine_adjective("AQ"), None);
        assert_eq!(cuisine_adjective("CR").as_deref(), Some("Costa Rican"));
    }

    #[test]
    fn maps_dishes_and_newer_countries() {
        for (tag, country) in [
            ("friture", "BE"),
            ("Frites", "BE"),
            ("gyros", "GR"),
            ("souvlaki", "GR"),
            ("paella", "ES"),
            ("pierogi", "PL"),
            ("korean_bbq", "KR"),
            ("tempura", "JP"),
            ("ceviche", "PE"),
            ("french_tacos", "FR"),
            ("heuriger", "AT"),
            ("poke", "US"),
            ("uruguayan", "UY"),
            ("guatemalan", "GT"),
            ("tanzanian", "TZ"),
            ("ugandan", "UG"),
            ("saudi", "SA"),
            ("luxembourgish", "LU"),
            ("macedonian", "MK"),
            ("kyrgyz", "KG"),
            ("malagasy", "MG"),
        ] {
            assert_eq!(countries_for_cuisine_tag(tag), vec![country], "{tag}");
        }
        // Still regional or global: no country.
        for tag in [
            "empanada", "arepa", "biryani", "shawarma", "fries", "waffle",
        ] {
            assert!(countries_for_cuisine_tag(tag).is_empty(), "{tag}");
        }
    }

    #[test]
    fn parses_multi_valued_tags() {
        assert_eq!(countries_for_cuisine_tag("italian;pizza"), vec!["IT"]);
        assert_eq!(
            countries_for_cuisine_tag("Thai; vietnamese"),
            vec!["TH", "VN"]
        );
        assert_eq!(countries_for_cuisine_tag("Sri Lankan"), vec!["LK"]);
        assert_eq!(
            countries_for_cuisine_tag("burger;regional"),
            [] as [&str; 0]
        );
    }
}
