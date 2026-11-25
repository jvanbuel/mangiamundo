//! Adding places just mapped nearby on OpenStreetMap.

use std::sync::atomic::Ordering;

use axum::http::StatusCode;

use crate::common::{call, session};
use crate::fake_osm::{app_with_osm, countries_here, osm_place};

#[tokio::test]
async fn places_just_mapped_nearby_are_added_and_confirm_pending_additions() {
    let (app, clock, fake, _db) = app_with_osm().await;
    let (alice, bob) = (session(&app).await, session(&app).await);
    // Alice added Inti, a Peruvian place, before a mapper put it on OSM.
    let body = r#"{"name":"Inti","country":"PE","lat":50.8801,"lng":4.7009,"note":false}"#;
    assert_eq!(
        call(&app, "POST", "/api/places", Some(&alice), Some(body))
            .await
            .0,
        StatusCode::OK
    );
    assert!(!countries_here(&app, &bob).await.contains(&"PE".to_string()));

    *fake.nearby.lock().unwrap() = vec![
        osm_place(42, "Lalibela", "ethiopian", 50.8798, 4.7005),
        osm_place(43, "Restaurant Inti", "peruvian", 50.8801, 4.7009),
        osm_place(44, "Far Away", "greek", 10.0, 10.0),
    ];
    let look = |token: String| {
        let app = app.clone();
        async move {
            call(
                &app,
                "POST",
                "/api/places/nearby/refresh?lat=50.88&lng=4.7",
                Some(&token),
                None,
            )
            .await
        }
    };
    let (status, body) = look(bob.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let added: Vec<&str> = body["added"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        added,
        vec!["Restaurant Inti"],
        "already listed, and outside every cached area, don't count"
    );
    assert_eq!(body["matched"], 1);
    assert!(
        countries_here(&app, &bob).await.contains(&"PE".to_string()),
        "for everyone now"
    );
    let (_, additions) = call(&app, "GET", "/api/additions", Some(&alice), None).await;
    assert_eq!(
        (
            additions[0]["status"].as_str(),
            additions[0]["place_id"].as_str()
        ),
        (Some("confirmed"), Some("osm-node-43"))
    );

    // Looking again finds nothing new.
    let (_, body) = look(bob.clone()).await;
    assert_eq!(
        (
            body["added"].as_array().unwrap().len(),
            body["matched"].as_u64()
        ),
        (0, Some(0))
    );

    // Five an hour, per user.
    for _ in 0..3 {
        assert_eq!(look(bob.clone()).await.0, StatusCode::OK);
    }
    assert_eq!(look(bob.clone()).await.0, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(look(alice.clone()).await.0, StatusCode::OK);
    clock.fetch_add(3601, Ordering::Relaxed);
    assert_eq!(look(bob.clone()).await.0, StatusCode::OK);
    let bad = call(
        &app,
        "POST",
        "/api/places/nearby/refresh?lat=91&lng=4.7",
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(bad.0, StatusCode::BAD_REQUEST);
}
