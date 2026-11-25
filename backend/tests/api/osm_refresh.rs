//! Refreshing a place from OpenStreetMap before the next import.

use std::sync::atomic::Ordering;

use axum::http::StatusCode;
use serde_json::Value;

use crate::common::{assert_error, call, session};
use crate::fake_osm::{app_with_osm, countries_here, osm_place};

#[tokio::test]
async fn a_place_can_be_refreshed_from_osm_before_the_next_import() {
    use mangiamundo::providers::Lookup;
    let (app, clock, fake, db) = app_with_osm().await;
    let token = session(&app).await;
    let refresh = |id: &'static str| {
        let (app, token) = (app.clone(), token.clone());
        async move {
            call(
                &app,
                "POST",
                &format!("/api/places/{id}/refresh"),
                Some(&token),
                None,
            )
            .await
        }
    };
    let set = |lookup: Lookup| {
        fake.places
            .lock()
            .unwrap()
            .insert("osm-node-42".into(), lookup);
    };

    // The same as we have.
    set(Lookup::Found(osm_place(
        42,
        "Lalibela",
        "ethiopian",
        50.8798,
        4.7005,
    )));
    let (status, body) = refresh("osm-node-42").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "unchanged");
    assert_eq!(body["place"]["name"], "Lalibela");

    // Hours added on OSM: in the place, and in the local copy the next area rebuild reads.
    let mut edited = osm_place(42, "Lalibela", "ethiopian", 50.8798, 4.7005);
    edited.opening_hours = Some("Mo-Su 12:00-22:00".into());
    set(Lookup::Found(edited));
    let (_, body) = refresh("osm-node-42").await;
    assert_eq!(body["status"], "updated");
    assert_eq!(body["place"]["hours"]["raw"], "Mo-Su 12:00-22:00");
    assert_eq!(
        body["place"]["hours"]["week"].as_array().map(Vec::len),
        Some(7),
        "with the week, like the details"
    );
    let copy: Option<String> =
        sqlx::query_scalar("SELECT opening_hours FROM osm_places WHERE id = 'osm-node-42'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(copy.as_deref(), Some("Mo-Su 12:00-22:00"));

    // Checked in, then the place closes on OSM: hidden from every list, visits kept.
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/places/osm-node-42/visits",
            Some(&token),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(countries_here(&app, &token)
        .await
        .contains(&"ET".to_string()));
    set(Lookup::Gone);
    let (_, body) = refresh("osm-node-42").await;
    assert_eq!(
        (body["status"].as_str(), &body["place"]),
        (Some("gone"), &Value::Null)
    );
    assert!(!countries_here(&app, &token)
        .await
        .contains(&"ET".to_string()));
    let (status, place) = call(&app, "GET", "/api/places/osm-node-42", Some(&token), None).await;
    assert_eq!(
        (status, place["visits"].as_array().unwrap().len()),
        (StatusCode::OK, 1)
    );
    let copies: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM osm_places")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(copies, 0, "the next area rebuild must not bring it back");

    // Reopened: listed again.
    set(Lookup::Found(osm_place(
        42,
        "Lalibela",
        "ethiopian",
        50.8798,
        4.7005,
    )));
    assert_eq!(refresh("osm-node-42").await.1["status"], "updated");
    assert!(countries_here(&app, &token)
        .await
        .contains(&"ET".to_string()));

    // Five a minute.
    assert_eq!(refresh("osm-node-42").await.0, StatusCode::OK);
    let (status, error) = refresh("osm-node-42").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_error(&error, "too_many_requests");
    clock.fetch_add(61, Ordering::Relaxed);

    // Moved out of Leuven on OSM: no longer listed there.
    set(Lookup::Found(osm_place(
        42,
        "Lalibela",
        "ethiopian",
        51.2194,
        4.4025,
    )));
    assert_eq!(refresh("osm-node-42").await.1["status"], "updated");
    assert!(!countries_here(&app, &token)
        .await
        .contains(&"ET".to_string()));

    // Only OSM places this user can see, and OSM must answer.
    assert_eq!(
        call(&app, "POST", "/api/places/osm-node-42/refresh", None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(refresh("user-abc").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(refresh("osm-node-999").await.0, StatusCode::NOT_FOUND);
    fake.down.store(true, Ordering::Relaxed);
    let (status, error) = refresh("osm-node-42").await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_error(&error, "upstream");
}
