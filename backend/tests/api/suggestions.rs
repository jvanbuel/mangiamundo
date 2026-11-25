//! Suggesting an open, unvisited place.

use axum::http::StatusCode;
use serde_json::Value;

use crate::common::{app, at, call, session, visits_uri, HERE};

#[tokio::test]
async fn suggests_an_open_unvisited_place_and_says_why_when_there_is_none() {
    let app = app().await;
    let token = session(&app).await;
    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}"), None, None).await;
    let places = places.as_array().unwrap();
    let is_open = |p: &Value| p["hours"]["status"]["open"] == true;
    let open: Vec<&Value> = places.iter().filter(|p| is_open(p)).collect();
    assert!(open.len() >= 2 && open.len() < places.len());

    // Visit one open place; it is never suggested.
    let visited = open[0];
    let (status, _) = call(
        &app,
        "POST",
        &visits_uri(visited),
        Some(&token),
        Some(&at(visited)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for _ in 0..20 {
        let (status, s) = call(
            &app,
            "GET",
            &format!("/api/suggestion?{HERE}"),
            Some(&token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_ne!(s["place"]["id"], visited["id"]);
        assert!(is_open(&s["place"]), "{s}");
        assert_eq!(
            s["place"]["visits"].as_array().unwrap().as_slice(),
            [] as [Value; 0]
        );
        assert_eq!(s["place"]["countries"][0], s["country"]);
    }

    // Excluding every open place leaves only closed ones, unless the open filter is dropped.
    let ids: Vec<&str> = open.iter().map(|p| p["id"].as_str().unwrap()).collect();
    let uri = format!("/api/suggestion?{HERE}&exclude={}", ids.join(","));
    let (status, body) = call(&app, "GET", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "nothing_open");
    let (status, s) = call(
        &app,
        "GET",
        &format!("{uri}&open=false"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!ids.contains(&s["place"]["id"].as_str().unwrap()));

    // Excluding everything leaves nothing.
    let all: Vec<&str> = places.iter().map(|p| p["id"].as_str().unwrap()).collect();
    let uri = format!(
        "/api/suggestion?{HERE}&open=false&exclude={}",
        all.join(",")
    );
    let (status, body) = call(&app, "GET", &uri, None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "nothing_left");
}
