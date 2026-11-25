//! The history of visits, and a place with its visits on its own.

use std::sync::atomic::Ordering;

use axum::http::StatusCode;
use serde_json::Value;

use crate::common::{
    app, app_with_clock, assert_error, at, call, session, visits_uri, DAY, HERE, START,
};

#[tokio::test]
async fn visits_are_a_history_with_one_visit_per_day() {
    let (app, clock) = app_with_clock().await;
    let token = session(&app).await;
    let (_, discover) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    let country = discover["countries"][0]["code"].as_str().unwrap();
    let (_, places) = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}&country={country}"),
        Some(&token),
        None,
    )
    .await;
    let place = &places[0];
    let uri = visits_uri(place);

    // On the spot, then again the same day from elsewhere: one visit, still verified.
    let (status, first) = call(&app, "POST", &uri, Some(&token), Some(&at(place))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["visit"]["at"], START);
    assert_eq!(first["visit"]["verified"], true);
    // With the 100 XP of the "taste a new country" quest.
    assert_eq!(first["profile"]["xp"], 175);
    let (_, same_day) = call(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(same_day["visit"], first["visit"]);

    // A day later: a second visit, unverified. The place still counts once.
    clock.fetch_add(DAY, Ordering::Relaxed);
    let (_, second) = call(&app, "POST", &uri, Some(&token), None).await;
    assert_ne!(second["visit"]["id"], first["visit"]["id"]);
    assert_eq!(second["visit"]["at"], START + DAY);
    assert_eq!(second["visit"]["verified"], false);
    assert_eq!(
        second["profile"], first["profile"],
        "a repeat visit earns no XP"
    );

    // The list carries the latest visit, the place all of them, newest first.
    let (_, listed) = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}&country={country}"),
        Some(&token),
        None,
    )
    .await;
    let listed = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == place["id"])
        .unwrap();
    assert_eq!(listed["visits"], serde_json::json!([second["visit"]]));
    let detail_uri = format!("/api/places/{}", place["id"].as_str().unwrap());
    let (status, detail) = call(&app, "GET", &detail_uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        detail["visits"],
        serde_json::json!([second["visit"], first["visit"]])
    );

    // Removing the latest visit leaves the place visited, and says what is left.
    let (status, removed) = call(
        &app,
        "DELETE",
        &format!("/api/visits/{}", second["visit"]["id"]),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(removed["visits"], serde_json::json!([first["visit"]]));
    assert_eq!(removed["profile"], first["profile"]);
    assert_eq!(removed["progress"], Value::Null, "no area, no progress");
    let (_, after) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(after["places_visited"], 1);
    let (_, detail) = call(&app, "GET", &detail_uri, Some(&token), None).await;
    assert_eq!(detail["visits"], serde_json::json!([first["visit"]]));

    // Removing it again: there is no such visit any more.
    let (status, body) = call(
        &app,
        "DELETE",
        &format!("/api/visits/{}", second["visit"]["id"]),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_error(&body, "not_found");

    // Nobody else can see or remove the visit.
    let other = session(&app).await;
    let (_, theirs) = call(&app, "GET", &detail_uri, Some(&other), None).await;
    assert_eq!(theirs["visits"], serde_json::json!([]));
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/visits/{}", first["visit"]["id"]),
        Some(&other),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // A wrong area leaves the visit in place.
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/visits/{}?area=demo-nowhere", first["visit"]["id"]),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, detail) = call(&app, "GET", &detail_uri, Some(&token), None).await;
    assert_eq!(detail["visits"], serde_json::json!([first["visit"]]));
}

#[tokio::test]
async fn a_place_can_be_fetched_on_its_own() {
    let app = app().await;
    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}"), None, None).await;
    let place = places
        .as_array()
        .unwrap()
        .iter()
        .find(|p| !p["hours"].is_null())
        .unwrap();
    let uri = format!("/api/places/{}", place["id"].as_str().unwrap());

    // Anonymously, and without a position the distance is 0.
    let (status, alone) = call(&app, "GET", &uri, None, None).await;
    assert_eq!(status, StatusCode::OK);
    for field in [
        "id",
        "name",
        "lat",
        "lng",
        "address",
        "url",
        "cuisine",
        "countries",
        "osm_url",
    ] {
        assert_eq!(alone[field], place[field], "{field}");
    }
    assert_eq!(alone["hours"]["raw"], place["hours"]["raw"]);
    assert_eq!(alone["hours"]["status"], place["hours"]["status"]);
    // Only the detail has the week, Monday to Sunday with one day marked as today.
    assert_eq!(place["hours"].get("week"), Some(&Value::Null));
    let week = alone["hours"]["week"].as_array().unwrap();
    assert_eq!(week.len(), 7);
    assert_eq!(week[0]["name"], "Monday");
    assert_eq!(week.iter().filter(|d| d["today"] == true).count(), 1);
    assert_eq!(alone["distance_m"], 0);
    assert_eq!(alone["visits"], serde_json::json!([]));

    let (_, near) = call(
        &app,
        "GET",
        &format!("{uri}?lat=50.8798&lng=4.7005"),
        None,
        None,
    )
    .await;
    assert!(near["distance_m"].as_u64().unwrap() > 0);

    let (status, body) = call(&app, "GET", "/api/places/does-not-exist", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");
}
