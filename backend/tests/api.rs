//! End-to-end tests of the HTTP API against the offline demo provider.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use mangiamundo::providers::demo::Demo;
use mangiamundo::{api, store, AppState};
use serde_json::Value;
use tower::ServiceExt;

const HERE: &str = "lat=50.8798&lng=4.7005";

async fn app() -> Router {
    api::router(AppState {
        db: store::connect("sqlite::memory:").await.unwrap(),
        places: Arc::new(Demo),
        geocoder: Arc::new(Demo),
        cache_ttl_secs: 3600,
    })
}

async fn call(app: &Router, method: &str, uri: &str, token: Option<&str>, body: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let body = match body {
        Some(json) => {
            request = request.header("content-type", "application/json");
            Body::from(json.to_string())
        }
        None => Body::empty(),
    };
    let response = app.clone().oneshot(request.body(body).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn session(app: &Router) -> String {
    let (status, body) = call(app, "POST", "/api/session", None, None).await;
    assert_eq!(status, StatusCode::OK);
    body["token"].as_str().unwrap().to_string()
}

fn badge<'a>(discover: &'a Value, id: &str) -> &'a Value {
    discover["badges"].as_array().unwrap().iter().find(|b| b["id"] == id).unwrap()
}

#[tokio::test]
async fn catalog_lists_countries_and_continents() {
    let app = app().await;
    let (status, body) = call(&app, "GET", "/api/countries", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["countries"].as_array().unwrap().len() > 200);
    assert_eq!(body["continents"].as_array().unwrap().len(), 6);
}

#[tokio::test]
async fn discover_works_anonymously_and_is_stable() {
    let app = app().await;
    let (status, first) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(first["world"]["available"].as_u64().unwrap() > 10);
    assert_eq!(first["world"]["discovered"], 0);
    assert_eq!(first["profile"]["xp"], 0);
    // A point a few hundred metres away lands in the same cached area.
    let (_, second) = call(&app, "GET", "/api/discover?lat=50.8810&lng=4.7030", None, None).await;
    assert_eq!(first["countries"], second["countries"]);
    assert_eq!(first["area"]["fetched_at"], second["area"]["fetched_at"]);
}

#[tokio::test]
async fn visiting_every_place_completes_a_country() {
    let app = app().await;
    let token = session(&app).await;
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), Some(&token), None).await;
    let country = discover["countries"][0]["code"].as_str().unwrap().to_string();

    let (status, places) =
        call(&app, "GET", &format!("/api/places?{HERE}&country={country}"), Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let places = places.as_array().unwrap().clone();
    assert_eq!(places.len() as u64, discover["countries"][0]["places"].as_u64().unwrap());
    assert!(places.iter().all(|p| p["countries"].as_array().unwrap().contains(&Value::from(country.clone()))));
    assert!(places.windows(2).all(|w| w[0]["distance_m"].as_u64() <= w[1]["distance_m"].as_u64()));

    // First check-in, made on the spot: verified, 10 XP for the visit + 50 for the country.
    let first = &places[0];
    let here = format!(r#"{{"lat":{},"lng":{}}}"#, first["lat"], first["lng"]);
    let (status, visit) =
        call(&app, "PUT", &format!("/api/visits/{}", first["id"].as_str().unwrap()), Some(&token), Some(&here)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(visit["verified"], true);
    assert_eq!(visit["profile"]["xp"], 60);

    for place in &places[1..] {
        let uri = format!("/api/visits/{}", place["id"].as_str().unwrap());
        let (status, visit) = call(&app, "PUT", &uri, Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(visit["verified"], false);
        // Checking in twice changes nothing.
        call(&app, "PUT", &uri, Some(&token), None).await;
    }

    let (_, after) = call(&app, "GET", &format!("/api/discover?{HERE}"), Some(&token), None).await;
    let entry = &after["countries"][0];
    assert_eq!(entry["visited"], entry["places"]);
    assert_eq!(after["world"]["discovered"], 1);
    assert_eq!(after["world"]["completed"], 1);
    assert_eq!(after["places_visited"].as_u64().unwrap(), places.len() as u64);
    assert_eq!(after["profile"]["visits"].as_u64().unwrap(), places.len() as u64);
    assert_eq!(badge(&after, "first-bite")["earned"], true);
    assert_eq!(badge(&after, "complete-1")["earned"], true);
    assert_eq!(badge(&after, "world")["earned"], false);

    // Another user sees none of it.
    let other = session(&app).await;
    let (_, theirs) = call(&app, "GET", &format!("/api/discover?{HERE}"), Some(&other), None).await;
    assert_eq!(theirs["world"]["discovered"], 0);

    // Undoing a visit takes the completion away again.
    let uri = format!("/api/visits/{}", first["id"].as_str().unwrap());
    let (status, _) = call(&app, "DELETE", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, undone) = call(&app, "GET", &format!("/api/discover?{HERE}"), Some(&token), None).await;
    assert_eq!(undone["world"]["completed"], 0);
}

#[tokio::test]
async fn rejects_bad_input() {
    let app = app().await;
    let (status, _) = call(&app, "GET", "/api/discover?lat=95&lng=4", None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&app, "GET", &format!("/api/discover?{HERE}&radius_km=500"), None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&app, "PUT", "/api/visits/demo-x", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(&app, "GET", &format!("/api/discover?{HERE}"), Some("nope"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let token = session(&app).await;
    let (status, _) = call(&app, "PUT", "/api/visits/does-not-exist", Some(&token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn geocoding_searches_and_reverses() {
    let app = app().await;
    let (status, found) = call(&app, "GET", "/api/geocode?q=leuv", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(found[0]["name"], "Leuven, Belgium");
    let (_, name) = call(&app, "GET", &format!("/api/geocode/reverse?{HERE}"), None, None).await;
    assert_eq!(name["name"], "Leuven");
}
