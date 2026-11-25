//! Places added by users, shown to whoever added them and sent to OSM.

use std::sync::atomic::Ordering;

use axum::http::StatusCode;
use mangiamundo::store;
use serde_json::Value;

use crate::common::{
    app_with_notes, assert_error, call, fake_osm_notes, session, DAY, HERE, START,
};

#[tokio::test]
async fn an_added_place_shows_up_for_whoever_added_it_and_is_sent_to_osm() {
    let (url, notes) = fake_osm_notes().await;
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (app, clock) = app_with_notes(db.clone(), Some(url), Some("secret".into()), START);
    let (alice, bob) = (session(&app).await, session(&app).await);
    let countries = |token: String| {
        let app = app.clone();
        async move {
            let (_, discover) = call(
                &app,
                "GET",
                &format!("/api/discover?{HERE}"),
                Some(&token),
                None,
            )
            .await;
            discover["countries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["code"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        }
    };
    assert!(!countries(alice.clone()).await.contains(&"NZ".to_string()));
    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}"), None, None).await;
    let (lat, lng) = (&places[0]["lat"], &places[0]["lng"]);
    let body = |name: &str| {
        format!(
            r#"{{"name":"{name}","country":"nz","lat":{lat},"lng":{lng},"address":" 1 Kiwi Lane ","website":"https://kiwi.example"}}"#
        )
    };

    assert_eq!(
        call(&app, "POST", "/api/places", None, Some(&body("Kiwi")))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let bad = format!(r#"{{"name":"Kiwi","country":"XX","lat":{lat},"lng":{lng}}}"#);
    assert_eq!(
        call(&app, "POST", "/api/places", Some(&alice), Some(&bad))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let blank = format!(r#"{{"name":"  ","country":"NZ","lat":{lat},"lng":{lng}}}"#);
    assert_eq!(
        call(&app, "POST", "/api/places", Some(&alice), Some(&blank))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );

    let (status, place) = call(
        &app,
        "POST",
        "/api/places",
        Some(&alice),
        Some(&body("Kiwi")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{place}");
    let id = place["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("user-"), "{id}");
    assert_eq!(place["cuisine"], "new_zealand");
    assert_eq!(place["address"], "1 Kiwi Lane");
    assert_eq!(place["countries"], serde_json::json!(["NZ"]));
    assert_eq!(place["osm_url"], Value::Null);

    assert!(countries(alice.clone()).await.contains(&"NZ".to_string()));
    assert!(
        !countries(bob.clone()).await.contains(&"NZ".to_string()),
        "pending places are only for whoever added them"
    );
    for token in [None, Some(bob.as_str())] {
        assert_eq!(
            call(&app, "GET", &format!("/api/places/{id}"), token, None)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/places/{id}"),
            Some(&alice),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/places/{id}/visits"),
            Some(&bob),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app,
            "PUT",
            &format!("/api/places/{id}/closed"),
            Some(&bob),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // A refresh of the area from the provider keeps it.
    clock.fetch_add(2 * 3600, Ordering::Relaxed);
    assert!(countries(alice.clone()).await.contains(&"NZ".to_string()));
    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/places/{id}/visits"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // The note is posted in the background; its id is kept last.
    let note_id = || {
        sqlx::query_scalar::<_, Option<i64>>("SELECT osm_note_id FROM places WHERE id = ?")
            .bind(&id)
            .fetch_one(&db)
    };
    for _ in 0..100 {
        if note_id().await.unwrap().is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(note_id().await.unwrap(), Some(7));
    {
        let notes = notes.lock().unwrap();
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(
            notes[0].starts_with("Bearer secret "),
            "as the Mangiamundo account: {}",
            notes[0]
        );
        assert!(
            notes[0].contains("amenity%3Drestaurant") && notes[0].contains("cuisine%3Dnew_zealand"),
            "{}",
            notes[0]
        );
        assert!(
            notes[0].contains("website%3Dhttps%3A%2F%2Fkiwi.example"),
            "{}",
            notes[0]
        );
    }

    // Someone adding it to OSM themselves gets no note.
    let own = format!(r#"{{"name":"B","country":"NZ","lat":{lat},"lng":{lng},"note":false}}"#);
    assert_eq!(
        call(&app, "POST", "/api/places", Some(&alice), Some(&own))
            .await
            .0,
        StatusCode::OK
    );
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(notes.lock().unwrap().len(), 1);

    // Five a day.
    for name in ["C", "D", "E"] {
        assert_eq!(
            call(&app, "POST", "/api/places", Some(&alice), Some(&body(name)))
                .await
                .0,
            StatusCode::OK
        );
    }
    let (status, error) = call(&app, "POST", "/api/places", Some(&alice), Some(&body("F"))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_error(&error, "too_many_requests");
    clock.fetch_add(DAY, Ordering::Relaxed);
    assert_eq!(
        call(&app, "POST", "/api/places", Some(&alice), Some(&body("F")))
            .await
            .0,
        StatusCode::OK
    );
}
