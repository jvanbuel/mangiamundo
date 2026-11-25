//! The app under test, on the demo provider and its own clock, and how tests talk to it.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use mangiamundo::providers::demo::Demo;
use mangiamundo::{api, store, AppState};
use serde_json::Value;
use tower::ServiceExt;

pub const HERE: &str = "area=demo-leuven";

/// 16 November 2026, 12:00 UTC.
pub const START: i64 = 1_794_830_400;
pub const DAY: i64 = 86_400;

pub async fn app() -> Router {
    app_with_clock().await.0
}

/// An app with its own clock, starting at START, that the test can move.
pub async fn app_with_clock() -> (Router, Arc<AtomicI64>) {
    let db = store::connect("sqlite::memory:").await.unwrap();
    app_with(db, None, START)
}

pub fn app_with(
    db: sqlx::SqlitePool,
    osm_notes_url: Option<String>,
    start: i64,
) -> (Router, Arc<AtomicI64>) {
    app_with_notes(db, osm_notes_url, None, start)
}

pub fn app_with_notes(
    db: sqlx::SqlitePool,
    osm_notes_url: Option<String>,
    osm_notes_token: Option<String>,
    start: i64,
) -> (Router, Arc<AtomicI64>) {
    let (state, now) = state_with_notes(db, osm_notes_url, osm_notes_token, start);
    (api::router(state), now)
}

/// The state of such an app, for a test that needs a hand in it.
pub fn state_with_notes(
    db: sqlx::SqlitePool,
    osm_notes_url: Option<String>,
    osm_notes_token: Option<String>,
    start: i64,
) -> (AppState, Arc<AtomicI64>) {
    let now = Arc::new(AtomicI64::new(start));
    let clock = now.clone();
    let state = AppState {
        loads: mangiamundo::areas::Loads::default(),
        limits: mangiamundo::limits::Limits::default(),
        db,
        places: Arc::new(Demo),
        areas: Arc::new(Demo),
        cache_ttl_secs: 3600,
        clock: Arc::new(move || clock.load(Ordering::Relaxed)),
        push_key: None,
        osm_notes_url,
        osm_notes_token,
        auth: Arc::default(),
    };
    (state, now)
}

pub async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<&str>,
) -> (StatusCode, Value) {
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
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

pub async fn session(app: &Router) -> String {
    let (status, body) = call(app, "POST", "/api/session", None, None).await;
    assert_eq!(status, StatusCode::OK);
    body["token"].as_str().unwrap().to_string()
}

/// Where to check in at a place from `/api/places`.
pub fn visits_uri(place: &Value) -> String {
    format!("/api/places/{}/visits", place["id"].as_str().unwrap())
}

/// A check-in body made at the place itself.
pub fn at(place: &Value) -> String {
    format!(
        r#"{{"position":{{"lat":{},"lng":{}}}}}"#,
        place["lat"], place["lng"]
    )
}

pub fn badge<'a>(discover: &'a Value, id: &str) -> &'a Value {
    discover["badges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == id)
        .unwrap()
}

/// Asserts a JSON error body with a message and the given code.
pub fn assert_error(body: &Value, code: &str) {
    assert_eq!(body["code"], code, "{body}");
    assert!(
        !body["error"].as_str().unwrap_or_default().is_empty(),
        "{body}"
    );
}

/// A fake OSM Notes API: its URL, and the notes posted to it as `<authorization> <form
/// body>`, with `-` for an anonymous note. Each note gets id 7.
pub async fn fake_osm_notes() -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
    use axum::routing::post;
    let notes: Arc<std::sync::Mutex<Vec<String>>> = Arc::default();
    let seen = notes.clone();
    let fake = Router::new().route(
        "/notes.json",
        post(
            move |headers: axum::http::HeaderMap, body: String| async move {
                let auth = headers
                    .get("authorization")
                    .map_or("-", |v| v.to_str().unwrap())
                    .to_string();
                seen.lock().unwrap().push(format!("{auth} {body}"));
                axum::Json(serde_json::json!({"type": "Feature", "properties": {"id": 7}}))
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/notes", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, fake).await.unwrap() });
    (url, notes)
}
