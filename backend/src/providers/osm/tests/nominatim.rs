use super::*;

#[test]
fn reads_photon_cities_and_postal_codes() {
    let found = serde_json::json!({"features": [
        {"geometry": {"coordinates": [4.70, 50.88]}, "properties":
         {"osm_type": "R", "osm_id": 3_366_463, "osm_value": "postcode", "type": "other", "name": "3000",
          "city": "Leuven", "state": "Flemish Brabant", "country": "Belgium"}},
        {"geometry": {"coordinates": [4.70, 50.88]}, "properties":
         {"osm_type": "N", "osm_id": 1, "osm_value": "sports_centre", "type": "house", "name": "Laser 3000"}},
        {"geometry": {"coordinates": [4.70, 50.88]}, "properties":
         {"osm_type": "R", "osm_id": 196_311, "osm_value": "city", "type": "city", "name": "Leuven",
          "state": "Flemish Brabant", "country": "Belgium"}},
        {"geometry": {"coordinates": [4.69, 50.88]}, "properties":
         {"osm_type": "R", "osm_id": 3_921_680, "osm_value": "administrative", "type": "district", "name": "Leuven"}}
    ]});
    let areas = photon_areas(&found);
    assert_eq!(areas.len(), 2);
    assert_eq!(
        (areas[0].id.as_str(), areas[0].name.as_str(), areas[0].kind),
        ("osm-relation-3366463", "3000 Leuven", AreaKind::PostalCode)
    );
    assert_eq!(
        (areas[1].id.as_str(), areas[1].name.as_str(), areas[1].kind),
        ("osm-relation-196311", "Leuven", AreaKind::City)
    );
    assert_eq!(areas[1].detail.as_deref(), Some("Flemish Brabant, Belgium"));
    assert_eq!((areas[1].lat, areas[1].lng), (50.88, 4.70));
}

#[test]
fn reads_cities_and_postal_codes_and_skips_other_results() {
    let found = serde_json::json!([
        {"osm_type": "relation", "osm_id": 196_311, "category": "boundary", "type": "administrative",
         "addresstype": "city", "name": "Leuven", "lat": "50.879", "lon": "4.701",
         "address": {"city": "Leuven", "state": "Flemish Brabant", "country": "Belgium"}},
        {"osm_type": "relation", "osm_id": 3_366_463, "category": "boundary", "type": "postal_code",
         "addresstype": "postcode", "name": "3000", "lat": "50.878", "lon": "4.702",
         "address": {"postcode": "3000", "city_district": "Leuven", "city": "Leuven", "country": "Belgium"}},
        {"osm_type": "relation", "osm_id": 3_921_680, "category": "boundary", "type": "administrative",
         "addresstype": "city_district", "name": "Leuven", "lat": "50.88", "lon": "4.70"},
        {"osm_type": "node", "osm_id": 1, "category": "place", "type": "city", "name": "Leuven", "lat": "50.88", "lon": "4.70"}
    ]);
    let areas: Vec<AreaInfo> = results(&found).filter_map(area_info).collect();
    assert_eq!(areas.len(), 2);
    assert_eq!(areas[0].id, "osm-relation-196311");
    assert_eq!(areas[0].kind, AreaKind::City);
    assert_eq!(areas[0].name, "Leuven");
    assert_eq!(areas[0].detail.as_deref(), Some("Flemish Brabant, Belgium"));
    assert_eq!(areas[1].kind, AreaKind::PostalCode);
    assert_eq!(areas[1].name, "3000 Leuven");
    assert_eq!(areas[1].detail.as_deref(), Some("Belgium"));
}

/// A fake Nominatim that finds nothing, after `delay`.
async fn slow_nominatim(delay: Duration) -> Nominatim {
    let app = axum::Router::new().route(
        "/search",
        axum::routing::get(move || async move {
            tokio::time::sleep(delay).await;
            "[]"
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Nominatim::new(reqwest::Client::new(), url)
}

#[tokio::test]
async fn a_lookup_that_waits_too_long_for_its_turn_is_told_to_try_again() {
    let nominatim = Nominatim {
        patience: Duration::from_millis(50),
        ..slow_nominatim(Duration::from_millis(400)).await
    };
    let (first, second) = tokio::join!(nominatim.search("Leuven"), async {
        // Behind the first, which is still being answered.
        tokio::time::sleep(Duration::from_millis(10)).await;
        nominatim.search("Gent").await
    });
    assert_eq!(first.unwrap(), [] as [AreaInfo; 0]);
    let error = second.unwrap_err();
    assert_eq!(error.downcast_ref::<Refused>(), Some(&Refused::Busy));
}

#[tokio::test]
async fn an_answer_that_takes_too_long_gives_the_turn_to_the_next() {
    let nominatim = Nominatim {
        timeout: Duration::from_millis(50),
        ..slow_nominatim(Duration::from_millis(400)).await
    };
    let started = Instant::now();
    let error = nominatim.search("Leuven").await.unwrap_err();
    assert!(error.downcast_ref::<Refused>().is_none(), "{error:#}");
    assert!(started.elapsed() < Duration::from_millis(300));
    assert!(nominatim.last.try_lock().is_ok());
}

/// Run by hand: `cargo test -- --ignored nominatim`.
#[tokio::test]
#[ignore = "talks to the public Nominatim server"]
async fn nominatim_finds_leuven_and_its_postal_codes() {
    let http = reqwest::Client::builder()
        .user_agent("mangiamundo-tests")
        .build()
        .unwrap();
    let nominatim = Nominatim::new(http, "https://nominatim.openstreetmap.org".into());
    let cities = nominatim.search("Leuven").await.unwrap();
    assert_eq!(cities[0].id, "osm-relation-196311", "{cities:?}");
    assert_eq!(cities[0].kind, AreaKind::City);
    let codes = nominatim.search("3000 Leuven").await.unwrap();
    assert!(
        codes
            .iter()
            .any(|a| a.id == "osm-relation-3366463" && a.name == "3000 Leuven"),
        "{codes:?}"
    );
    let here = nominatim.locate(50.8790, 4.7005).await.unwrap();
    assert_eq!(here.city.unwrap().id, "osm-relation-196311");
    assert_eq!(here.postal_code.unwrap().id, "osm-relation-3366463");
    let street = nominatim.search("Naamsestraat, Leuven").await.unwrap();
    assert_eq!(
        street.first().map(|a| a.id.as_str()),
        Some("osm-relation-196311"),
        "{street:?}"
    );
    let boundary = nominatim
        .boundary("osm-relation-3366463")
        .await
        .unwrap()
        .unwrap();
    assert!(boundary.shape.contains(50.8790, 4.7005));
    assert!(
        !boundary.shape.contains(50.90, 4.73),
        "Wilsele is 3012, not 3000"
    );
    assert!(
        nominatim
            .boundary("osm-relation-52411")
            .await
            .unwrap()
            .is_none(),
        "Belgium is not a city"
    );
}
