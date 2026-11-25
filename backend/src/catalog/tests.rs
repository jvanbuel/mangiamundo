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
