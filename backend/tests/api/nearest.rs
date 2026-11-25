//! The nearest places serving a country's food, whatever area they are in.

use axum::http::StatusCode;
use serde_json::Value;

use crate::common::{app, assert_error, at, call, session, visits_uri, HERE};

#[tokio::test]
async fn lists_the_nearest_places_of_a_country_from_any_area() {
    let app = app().await;
    let token = session(&app).await;
    // Leuven's places are known once its area has been played.
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    let country = discover["countries"][0]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (_, places) = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}&country={country}"),
        None,
        None,
    )
    .await;
    let first = &places[0];
    let from = format!("lat={}&lng={}", first["lat"], first["lng"]);
    let uri = format!("/api/countries/{}/nearest?{from}", country.to_lowercase());

    let (status, before) = call(&app, "GET", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{before}");
    let before = before.as_array().unwrap();
    assert_eq!(before.len(), places.as_array().unwrap().len().min(10));
    assert_eq!(before[0]["id"], first["id"]);
    assert_eq!(before[0]["distance_m"], 0);
    let distances: Vec<u64> = before
        .iter()
        .map(|p| p["distance_m"].as_u64().unwrap())
        .collect();
    assert!(distances.is_sorted(), "{distances:?}");
    assert!(before.iter().all(|p| p["countries"]
        .as_array()
        .unwrap()
        .contains(&Value::from(country.as_str()))));

    // A check-in shows on the list, like on the area's.
    let (status, _) = call(
        &app,
        "POST",
        &visits_uri(first),
        Some(&token),
        Some(&at(first)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, after) = call(&app, "GET", &format!("{uri}&limit=1"), Some(&token), None).await;
    assert_eq!(after.as_array().unwrap().len(), 1);
    assert_eq!(after[0]["visits"][0]["verified"], true);

    // A country nobody serves around here is an empty list; an unknown one is a 404.
    let served: Vec<&str> = discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["code"].as_str().unwrap())
        .collect();
    let (_, catalog) = call(&app, "GET", "/api/countries", None, None).await;
    let unserved = catalog["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["code"].as_str().unwrap())
        .find(|c| !served.contains(c))
        .unwrap();
    let (status, none) = call(
        &app,
        "GET",
        &format!("/api/countries/{unserved}/nearest?{from}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(none, Value::Array(vec![]));
    let (status, body) = call(
        &app,
        "GET",
        &format!("/api/countries/XX/nearest?{from}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_error(&body, "not_found");
    let (status, body) = call(
        &app,
        "GET",
        &format!("/api/countries/{country}/nearest"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_error(&body, "bad_request");
    let (status, body) = call(
        &app,
        "GET",
        &format!("/api/countries/{country}/nearest?lat=91&lng=0"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_error(&body, "bad_request");
}
