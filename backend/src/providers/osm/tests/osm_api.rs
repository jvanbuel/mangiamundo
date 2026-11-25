use super::*;

#[test]
fn osm_api_answers_become_places() {
    let node = r#"{"version":"0.6","elements":[{"type":"node","id":7,"lat":50.88,"lon":4.70,"tags":{"amenity":"restaurant","name":"Lalibela","cuisine":"ethiopian"}}]}"#;
    let Lookup::Found(place) = parse_api("node", 7, node).unwrap() else {
        panic!("not found")
    };
    assert_eq!(
        (place.id.as_str(), place.lat, place.countries.clone()),
        ("osm-node-7", 50.88, vec!["ET"])
    );

    let way = r#"{"elements":[
        {"type":"node","id":1,"lat":50.0,"lon":4.0},{"type":"node","id":2,"lat":50.2,"lon":4.4},
        {"type":"way","id":9,"nodes":[1,2],"tags":{"amenity":"cafe","name":"Kissa","cuisine":"japanese"}}]}"#;
    let Lookup::Found(place) = parse_api("way", 9, way).unwrap() else {
        panic!("not found")
    };
    assert_eq!(
        (place.id.as_str(), place.lat, place.lng),
        ("osm-way-9", 50.1, 4.2)
    );

    // Across the antimeridian: an error, so Overpass answers instead.
    let wrapped = way
        .replace(r#""lon":4.0"#, r#""lon":179.999"#)
        .replace(r#""lon":4.4"#, r#""lon":-179.999"#);
    assert!(parse_api("way", 9, &wrapped).is_err());

    // No longer a restaurant, or closed: gone, as the Overpass query would say.
    let shop = node.replace("restaurant", "shop");
    assert_eq!(parse_api("node", 7, &shop).unwrap(), Lookup::Gone);
    let closed = node.replace(r#""cuisine""#, r#""disused":"yes","cuisine""#);
    assert_eq!(parse_api("node", 7, &closed).unwrap(), Lookup::Gone);
}

/// Run by hand: `cargo test -- --ignored osm_api_reads`.
#[tokio::test]
#[ignore = "talks to the public OpenStreetMap API"]
async fn osm_api_reads_a_place_quickly() {
    let http = reqwest::Client::builder()
        .user_agent("mangiamundo-tests")
        .build()
        .unwrap();
    // Unreachable Overpass: the answer has to come from the OSM API.
    let overpass = Overpass::new(http, vec!["http://127.0.0.1:9/api/interpreter".into()]);
    let started = Instant::now();
    // An Italian restaurant in Leuven.
    let lookup = overpass.place("osm-node-365439479").await.unwrap();
    assert!(matches!(lookup, Lookup::Found(_)), "{lookup:?}");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
}
