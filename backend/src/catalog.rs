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

/// A signature dish of a country. Logging one at a check-in earns bonus XP.
#[derive(Debug, Clone, Serialize)]
pub struct Dish {
    /// Country code plus a slug of the name, e.g. `jp-ramen`. Stable as long as the name is.
    pub id: String,
    pub country: String,
    pub name: String,
}

/// All signature dishes from `data/dishes.json`, in file order per country.
pub fn dishes() -> &'static [Dish] {
    static DATA: OnceLock<Vec<Dish>> = OnceLock::new();
    DATA.get_or_init(|| {
        let raw: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(include_str!("../data/dishes.json")).expect("data/dishes.json is valid");
        raw.into_iter()
            .flat_map(|(country, names)| {
                let names: Vec<String> = serde_json::from_value(names).expect("dish names are strings");
                names.into_iter().map(move |name| Dish {
                    id: format!("{}-{}", country.to_lowercase(), slug(&name)),
                    country: country.clone(),
                    name,
                })
            })
            .collect()
    })
}

pub fn dish(id: &str) -> Option<&'static Dish> {
    dishes().iter().find(|d| d.id == id)
}

/// Lowercase ASCII slug: accents are folded, everything else becomes a single dash.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars().flat_map(char::to_lowercase) {
        let folded = match ch {
            'a'..='z' | '0'..='9' => Some(ch.to_string()),
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ả' => Some("a".into()),
            'æ' => Some("ae".into()),
            'ç' | 'ć' | 'č' => Some("c".into()),
            'đ' => Some("d".into()),
            'è' | 'é' | 'ê' | 'ë' | 'ē' => Some("e".into()),
            'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ı' => Some("i".into()),
            'ł' => Some("l".into()),
            'ñ' => Some("n".into()),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ơ' | 'ở' => Some("o".into()),
            'œ' => Some("oe".into()),
            'š' | 'ś' => Some("s".into()),
            'ù' | 'ú' | 'û' | 'ü' | 'ū' => Some("u".into()),
            'ž' | 'ż' | 'ź' => Some("z".into()),
            // The dot of a lowercased 'İ'.
            '\u{307}' => Some(String::new()),
            _ => None,
        };
        match folded {
            Some(text) => out.push_str(&text),
            None if !out.is_empty() && !out.ends_with('-') => out.push('-'),
            None => {}
        }
    }
    out.trim_end_matches('-').to_string()
}

/// One national cuisine and how each data source refers to it.
pub struct Cuisine {
    pub country: &'static str,
    /// Values of the OpenStreetMap `cuisine=*` tag. The first one is the canonical demonym.
    pub osm: &'static [&'static str],
    /// Google Places (New) primary type, when Google has one for this cuisine.
    pub google: Option<&'static str>,
}

const fn c(
    country: &'static str,
    osm: &'static [&'static str],
    google: Option<&'static str>,
) -> Cuisine {
    Cuisine { country, osm, google }
}

/// Only cuisines that point at a single country are listed. Regional labels such as
/// "asian", "african" or "mediterranean" are deliberately left out, and so are dishes
/// that have gone global (pizza, kebab, burger): they say little about a country.
pub const CUISINES: &[Cuisine] = &[
    // Europe
    c("AL", &["albanian"], None),
    c("AT", &["austrian"], Some("austrian_restaurant")),
    c("BA", &["bosnian"], None),
    c("BE", &["belgian"], Some("belgian_restaurant")),
    c("BG", &["bulgarian"], None),
    c("BY", &["belarusian"], None),
    c("CH", &["swiss"], Some("swiss_restaurant")),
    c("CY", &["cypriot"], None),
    c("CZ", &["czech"], Some("czech_restaurant")),
    c("DE", &["german", "bavarian"], Some("german_restaurant")),
    c("DK", &["danish"], Some("danish_restaurant")),
    c("EE", &["estonian"], None),
    c("ES", &["spanish", "tapas", "basque", "catalan"], Some("spanish_restaurant")),
    c("FI", &["finnish"], None),
    c("FR", &["french", "crepe"], Some("french_restaurant")),
    c("GB", &["british", "english", "scottish", "fish_and_chips"], Some("british_restaurant")),
    c("GE", &["georgian"], None),
    c("GR", &["greek"], Some("greek_restaurant")),
    c("HR", &["croatian"], Some("croatian_restaurant")),
    c("HU", &["hungarian"], Some("hungarian_restaurant")),
    c("IE", &["irish"], Some("irish_restaurant")),
    c("IS", &["icelandic"], None),
    c("IT", &["italian", "italian_pizza", "pasta"], Some("italian_restaurant")),
    c("LT", &["lithuanian"], None),
    c("LV", &["latvian"], None),
    c("MT", &["maltese"], None),
    c("NL", &["dutch"], Some("dutch_restaurant")),
    c("NO", &["norwegian"], None),
    c("PL", &["polish"], Some("polish_restaurant")),
    c("PT", &["portuguese"], Some("portuguese_restaurant")),
    c("RO", &["romanian"], Some("romanian_restaurant")),
    c("RS", &["serbian"], None),
    c("RU", &["russian"], Some("russian_restaurant")),
    c("SE", &["swedish"], None),
    c("SI", &["slovenian"], None),
    c("SK", &["slovak"], None),
    c("UA", &["ukrainian"], Some("ukrainian_restaurant")),
    // Asia
    c("AF", &["afghan"], Some("afghani_restaurant")),
    c("AM", &["armenian"], None),
    c("AZ", &["azerbaijani"], None),
    c("BD", &["bangladeshi"], Some("bangladeshi_restaurant")),
    c("CN", &["chinese", "sichuan", "cantonese", "dim_sum", "hotpot"], Some("chinese_restaurant")),
    c("ID", &["indonesian"], Some("indonesian_restaurant")),
    c("IL", &["israeli"], Some("israeli_restaurant")),
    c("IN", &["indian", "south_indian", "punjabi"], Some("indian_restaurant")),
    c("IQ", &["iraqi"], None),
    c("IR", &["persian", "iranian"], Some("persian_restaurant")),
    c("JO", &["jordanian"], None),
    c("JP", &["japanese", "sushi", "ramen", "udon", "soba", "yakitori", "okonomiyaki"], Some("japanese_restaurant")),
    c("KH", &["cambodian"], Some("cambodian_restaurant")),
    c("KR", &["korean"], Some("korean_restaurant")),
    c("KZ", &["kazakh"], None),
    c("LA", &["lao", "laotian"], None),
    c("LB", &["lebanese"], Some("lebanese_restaurant")),
    c("LK", &["sri_lankan"], Some("sri_lankan_restaurant")),
    c("MM", &["burmese"], Some("burmese_restaurant")),
    c("MN", &["mongolian"], Some("mongolian_barbecue_restaurant")),
    c("MY", &["malaysian", "malay"], Some("malaysian_restaurant")),
    c("NP", &["nepalese", "nepali"], None),
    c("PH", &["filipino"], Some("filipino_restaurant")),
    c("PK", &["pakistani"], Some("pakistani_restaurant")),
    c("PS", &["palestinian"], None),
    c("SG", &["singaporean"], None),
    c("SY", &["syrian"], None),
    c("TH", &["thai"], Some("thai_restaurant")),
    c("TR", &["turkish"], Some("turkish_restaurant")),
    c("TW", &["taiwanese", "bubble_tea"], Some("taiwanese_restaurant")),
    c("UZ", &["uzbek"], None),
    c("VN", &["vietnamese", "pho"], Some("vietnamese_restaurant")),
    c("YE", &["yemeni"], None),
    // Africa
    c("CD", &["congolese"], None),
    c("CI", &["ivorian"], None),
    c("CM", &["cameroonian"], None),
    c("DZ", &["algerian"], None),
    c("EG", &["egyptian"], None),
    c("ER", &["eritrean"], None),
    c("ET", &["ethiopian"], Some("ethiopian_restaurant")),
    c("GH", &["ghanaian"], None),
    c("KE", &["kenyan"], None),
    c("MA", &["moroccan"], Some("moroccan_restaurant")),
    c("NG", &["nigerian"], None),
    c("SD", &["sudanese"], None),
    c("SN", &["senegalese"], None),
    c("SO", &["somali"], None),
    c("TN", &["tunisian"], Some("tunisian_restaurant")),
    c("ZA", &["south_african"], None),
    // North America
    c("CA", &["canadian"], None),
    c("CU", &["cuban"], Some("cuban_restaurant")),
    c("DO", &["dominican"], None),
    c("HT", &["haitian"], None),
    c("JM", &["jamaican"], None),
    c("MX", &["mexican", "tex-mex", "tacos"], Some("mexican_restaurant")),
    c("SV", &["salvadoran"], None),
    c("US", &["american", "cajun", "hawaiian", "diner"], Some("american_restaurant")),
    // South America
    c("AR", &["argentinian", "argentine"], Some("argentinian_restaurant")),
    c("BO", &["bolivian"], None),
    c("BR", &["brazilian"], Some("brazilian_restaurant")),
    c("CL", &["chilean"], None),
    c("CO", &["colombian"], Some("colombian_restaurant")),
    c("EC", &["ecuadorian"], None),
    c("PE", &["peruvian"], Some("peruvian_restaurant")),
    c("SR", &["surinamese"], None),
    c("VE", &["venezuelan"], None),
    // Oceania
    c("AU", &["australian"], Some("australian_restaurant")),
    c("NZ", &["new_zealand"], None),
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
    fn every_cuisine_has_signature_dishes_with_unique_ids() {
        let mut ids = std::collections::HashSet::new();
        for dish in dishes() {
            assert!(country(&dish.country).is_some(), "unknown country {}", dish.country);
            assert!(dish.id.len() > 3 && !dish.id.contains("--"), "bad id {}", dish.id);
            assert!(ids.insert(dish.id.clone()), "duplicate dish {}", dish.id);
        }
        for cu in CUISINES {
            assert!(dishes().iter().any(|d| d.country == cu.country), "no dishes for {}", cu.country);
        }
        assert_eq!(dish("jp-ramen").map(|d| d.name.as_str()), Some("Ramen"));
        assert_eq!(slug("Bœuf bourguignon"), "boeuf-bourguignon");
        assert_eq!(slug("İskender kebap"), "iskender-kebap");
        assert_eq!(slug("Phở"), "pho");
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
