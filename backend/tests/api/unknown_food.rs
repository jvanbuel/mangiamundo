//! Restaurants whose food is unknown, given a country or none.

use axum::http::StatusCode;
use mangiamundo::store;

use crate::common::{app_with, call, fake_osm_notes, session, HERE, START};

#[tokio::test]
async fn restaurants_with_unknown_food_can_be_given_a_country_or_none() {
    let (url, notes) = fake_osm_notes().await;
    let db = store::connect("sqlite::memory:").await.unwrap();
    let unknown = |id: &str, name: &str, lat: f64| store::UnknownPlace {
        id: id.into(),
        name: name.into(),
        lat,
        lng: 4.7005,
        address: None,
        distance_m: 0,
    };
    store::replace_unknown(
        &db,
        &[
            unknown("osm-node-1", "Lalibela", 50.8798),
            unknown("osm-node-2", "Frituur Nr 1", 50.8790),
            unknown("osm-node-3", "Far away", 10.0),
        ],
    )
    .await
    .unwrap();
    let (app, _) = app_with(db, Some(url), START);
    let (alice, bob, carol) = (
        session(&app).await,
        session(&app).await,
        session(&app).await,
    );
    let list = |token: String| {
        let app = app.clone();
        async move {
            let uri = format!("/api/unknown-places?{HERE}&lat=50.8798&lng=4.7005");
            let (status, body) = call(&app, "GET", &uri, Some(&token), None).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            body.as_array()
                .unwrap()
                .iter()
                .map(|p| p["id"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(
        list(alice.clone()).await,
        ["osm-node-1", "osm-node-2"],
        "nearest first, inside the area"
    );

    let suggest = |token: String, id: &str, country: &str| {
        let app = app.clone();
        let (uri, body) = (
            format!("/api/unknown-places/{id}/cuisine"),
            format!(r#"{{"country":"{country}"}}"#),
        );
        async move { call(&app, "POST", &uri, Some(&token), Some(&body)).await }
    };
    assert_eq!(
        suggest(alice.clone(), "osm-node-1", "XX").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        suggest(alice.clone(), "osm-node-9", "ET").await.0,
        StatusCode::NOT_FOUND
    );
    let (status, added) = suggest(alice.clone(), "osm-node-1", "et").await;
    assert_eq!(status, StatusCode::OK, "{added}");
    assert_eq!(added["name"], "Lalibela");
    assert_eq!(added["countries"], serde_json::json!(["ET"]));
    assert!(added["id"].as_str().unwrap().starts_with("user-"));
    assert_eq!(list(alice.clone()).await, ["osm-node-2"]);
    assert_eq!(
        list(bob.clone()).await,
        ["osm-node-1", "osm-node-2"],
        "only alice answered"
    );

    // Nothing specific: gone for whoever said so, and for everyone after two.
    let none = |token: String| {
        let app = app.clone();
        async move {
            call(
                &app,
                "PUT",
                "/api/unknown-places/osm-node-2/no-cuisine",
                Some(&token),
                None,
            )
            .await
            .0
        }
    };
    assert_eq!(none(bob.clone()).await, StatusCode::NO_CONTENT);
    assert_eq!(list(bob.clone()).await, ["osm-node-1"]);
    assert_eq!(list(carol.clone()).await, ["osm-node-1", "osm-node-2"]);
    assert_eq!(none(alice.clone()).await, StatusCode::NO_CONTENT);
    assert_eq!(list(carol.clone()).await, ["osm-node-1"]);

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let notes = notes.lock().unwrap();
    assert_eq!(
        notes.len(),
        1,
        "only the suggestion posts a note: {notes:?}"
    );
    assert!(notes[0].contains("cuisine%3Dethiopian"), "{}", notes[0]);
    assert!(
        notes[0].contains("openstreetmap.org%2Fnode%2F1"),
        "{}",
        notes[0]
    );
}
