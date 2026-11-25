use super::*;

#[test]
fn a_failed_query_is_an_error_not_an_empty_answer() {
    let failed = r#"{"elements":[],"remark":"runtime error: Query timed out in \"query\" at line 1 after 26 seconds."}"#;
    assert!(parse(failed).is_err());
    assert!(
        parse(r#"{"elements":[]}"#).unwrap().is_empty(),
        "really nothing there"
    );
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
    assert_eq!(
        places[0].address.as_deref(),
        Some("Naamsestraat 12, Leuven")
    );
    assert_eq!(places[1].id, "osm-way-2");
    assert_eq!(places[1].countries, vec!["VN", "TH"]);
    assert_eq!((places[1].lat, places[1].lng), (50.87, 4.71));
    assert_eq!(places[1].address, None);
}

#[test]
fn restaurants_without_a_cuisine_are_unknown_not_places() {
    let body = r#"{"elements":[
        {"type":"node","id":1,"lat":50.88,"lon":4.70,"tags":{"name":"Lalibela","addr:city":"Leuven"}},
        {"type":"node","id":2,"lat":50.88,"lon":4.70,"tags":{"name":"Shut","disused":"yes"}},
        {"type":"node","id":3,"lat":50.88,"lon":4.70,"tags":{"name":"Burger Bar","cuisine":"burger"}},
        {"type":"node","id":4,"lat":50.88,"lon":4.70,"tags":{}}
    ]}"#;
    let all = elements(body).unwrap();
    let unknown = unknown(&all);
    assert_eq!(unknown.len(), 1);
    assert_eq!(
        (unknown[0].id.as_str(), unknown[0].address.as_deref()),
        ("osm-node-1", Some("Leuven"))
    );
    assert_eq!(places(all), [] as [FoundPlace; 0]);
    let q = import_query(IMPORT_BBOX, 300);
    assert!(
        q.contains("[!\"cuisine\"][\"name\"](49.4,2.4,51.6,6.5)"),
        "{q}"
    );
}

#[test]
fn recorded_closures_are_skipped() {
    let tags = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    let today = "2026-11-16";
    assert!(is_closed(&tags(&[("opening_hours", "closed")]), today));
    assert!(is_closed(&tags(&[("end_date", "2025")]), today));
    assert!(is_closed(&tags(&[("end_date", "2026-11-01")]), today));
    assert!(!is_closed(&tags(&[("end_date", "2027-01")]), today));
    assert!(!is_closed(
        &tags(&[("opening_hours", "Mo-Fr 11:00-22:00")]),
        today
    ));
    // Disused or abandoned, with the live tags still on.
    assert!(is_closed(
        &tags(&[("amenity", "fast_food"), ("disused", "yes")]),
        today
    ));
    assert!(is_closed(
        &tags(&[("amenity", "fast_food"), ("abandoned", "yes")]),
        today
    ));
    assert!(!is_closed(
        &tags(&[("amenity", "fast_food"), ("disused", "no")]),
        today
    ));
    for prefix in ["disused", "abandoned", "was"] {
        let key = format!("{prefix}:amenity");
        assert!(
            is_closed(
                &tags(&[("amenity", "fast_food"), (&key, "fast_food")]),
                today
            ),
            "{key}"
        );
        // What it used to be, before it became what it is.
        assert!(
            !is_closed(&tags(&[("amenity", "restaurant"), (&key, "cafe")]), today),
            "{key}"
        );
    }
}
