use std::collections::HashMap;

use super::card::{card_svg, fit};
use super::*;
use crate::game::Status;

#[test]
fn card_renders_a_png_with_the_world_and_text() {
    let statuses = HashMap::from([
        ("PE", Status::Discovered),
        ("JP", Status::Completed),
        ("BE", Status::Available),
    ]);
    let svg = card_svg(
        &statuses,
        "2 of 3 countries tasted",
        "Brussels & co · Level 2",
    );
    assert!(
        svg.contains("fill=\"#e0a100\""),
        "tasted countries are gold"
    );
    assert!(svg.contains("Brussels &amp; co"), "text is escaped");
    let png = card_png(&statuses, "2 of 3 countries tasted", "Brussels · Level 2").unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    assert!(png.len() > 10_000, "a real picture, not a blank one");
}

#[test]
fn visit_headlines_name_the_food() {
    let visit = |countries: &[&str]| SharedVisit {
        place: "Casa".into(),
        countries: countries
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
        date: "2026-11-22".into(),
    };
    assert_eq!(visit_headline(&visit(&["PE"])), "Peruvian food at Casa");
    assert_eq!(visit_headline(&visit(&[])), "Dinner at Casa");
    assert_eq!(pretty_date("2026-11-02"), "2 Nov 2026");
    assert_eq!(fit("Short", 10), "Short");
    assert_eq!(fit("A very long restaurant name", 10), "A very lo…");
}
