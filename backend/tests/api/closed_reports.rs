//! Reports that a place has closed, and the OSM notes they lead to.

use axum::http::StatusCode;
use mangiamundo::geo::{Bbox, Shape};
use mangiamundo::providers::{AreaInfo, AreaKind, Boundary};
use mangiamundo::store;
use serde_json::Value;

use crate::common::{app, app_with, call, fake_osm_notes, session, HERE, START};

#[tokio::test]
async fn closed_reports_hide_a_place_from_the_reporter_then_from_everyone() {
    let app = app().await;
    let (alice, bob) = (session(&app).await, session(&app).await);
    let ids = |places: &Value| -> Vec<String> {
        places
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap().to_string())
            .collect()
    };
    let list = |token: Option<String>| {
        let app = app.clone();
        async move {
            ids(&call(
                &app,
                "GET",
                &format!("/api/places?{HERE}"),
                token.as_deref(),
                None,
            )
            .await
            .1)
        }
    };
    let place = list(None).await[0].clone();
    let report = format!("/api/places/{place}/closed");

    assert_eq!(
        call(&app, "PUT", &report, None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "PUT", "/api/places/nope/closed", Some(&alice), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    let (status, reported) = call(&app, "PUT", &report, Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reported["progress"], Value::Null, "no area, no progress");
    // Reporting twice counts once. With the area, the answer has the progress without it.
    let (status, reported) =
        call(&app, "PUT", &format!("{report}?{HERE}"), Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK);
    let (_, mut discover) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&alice),
        None,
    )
    .await;
    discover.as_object_mut().unwrap().remove("area");
    assert_eq!(reported["progress"], discover);
    let (_, theirs) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(
        discover["places"].as_u64().unwrap() + 1,
        theirs["places"].as_u64().unwrap()
    );
    assert!(!list(Some(alice.clone())).await.contains(&place));
    assert!(list(Some(bob.clone())).await.contains(&place));
    assert!(list(None).await.contains(&place));

    call(&app, "PUT", &report, Some(&bob), None).await;
    assert!(!list(None).await.contains(&place));
}

#[tokio::test]
async fn the_second_closed_report_of_an_osm_place_posts_one_osm_note() {
    let (url, notes) = fake_osm_notes().await;

    // An OSM place already saved, so the demo provider is not asked.
    let db = store::connect("sqlite::memory:").await.unwrap();
    let place = mangiamundo::providers::FoundPlace {
        id: "osm-node-42".into(),
        name: "Lalibela".into(),
        lat: 50.8798,
        lng: 4.7005,
        address: None,
        url: None,
        cuisine: Some("ethiopian".into()),
        opening_hours: None,
        countries: vec!["ET"],
    };
    let area = store::Area {
        boundary: Boundary {
            info: AreaInfo {
                id: "demo-leuven".into(),
                name: "Leuven".into(),
                kind: AreaKind::City,
                detail: None,
                lat: 50.88,
                lng: 4.7,
            },
            shape: Shape::rect(Bbox {
                south: 50.8,
                west: 4.6,
                north: 50.95,
                east: 4.8,
            }),
        },
        provider: "demo".into(),
        fetched_at: START,
    };
    store::save_area(&db, &area, &[place]).await.unwrap();
    let (app, _) = app_with(db, Some(url), START);

    let users = [
        session(&app).await,
        session(&app).await,
        session(&app).await,
    ];
    for user in &users {
        assert_eq!(
            call(
                &app,
                "PUT",
                "/api/places/osm-node-42/closed",
                Some(user),
                None
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let notes = notes.lock().unwrap();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].starts_with("- "),
        "anonymous without a token: {}",
        notes[0]
    );
    assert!(
        notes[0].contains("lat=50.8798") && notes[0].contains("lon=4.7005"),
        "{}",
        notes[0]
    );
    assert!(
        notes[0].contains("openstreetmap.org%2Fnode%2F42"),
        "{}",
        notes[0]
    );
}
