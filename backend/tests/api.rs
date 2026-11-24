//! End-to-end tests of the HTTP API against the offline demo provider.

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

const HERE: &str = "area=demo-leuven";

/// 16 November 2026, 12:00 UTC.
const START: i64 = 1_794_830_400;
const DAY: i64 = 86_400;

async fn app() -> Router {
    app_with_clock().await.0
}

/// An app with its own clock, starting at START, that the test can move.
async fn app_with_clock() -> (Router, Arc<AtomicI64>) {
    let db = store::connect("sqlite::memory:").await.unwrap();
    app_with(db, None, START).await
}

async fn app_with(
    db: sqlx::SqlitePool,
    osm_notes_url: Option<String>,
    start: i64,
) -> (Router, Arc<AtomicI64>) {
    app_with_notes(db, osm_notes_url, None, start).await
}

async fn app_with_notes(
    db: sqlx::SqlitePool,
    osm_notes_url: Option<String>,
    osm_notes_token: Option<String>,
    start: i64,
) -> (Router, Arc<AtomicI64>) {
    let now = Arc::new(AtomicI64::new(start));
    let clock = now.clone();
    let router = api::router(AppState {
        loads: Default::default(),
        limits: Default::default(),
        db,
        places: Arc::new(Demo),
        areas: Arc::new(Demo),
        cache_ttl_secs: 3600,
        clock: Arc::new(move || clock.load(Ordering::Relaxed)),
        push_key: None,
        osm_notes_url,
        osm_notes_token,
        auth: Default::default(),
    });
    (router, now)
}

async fn call(
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

async fn session(app: &Router) -> String {
    let (status, body) = call(app, "POST", "/api/session", None, None).await;
    assert_eq!(status, StatusCode::OK);
    body["token"].as_str().unwrap().to_string()
}

/// Where to check in at a place from `/api/places`.
fn visits_uri(place: &Value) -> String {
    format!("/api/places/{}/visits", place["id"].as_str().unwrap())
}

/// A check-in body made at the place itself.
fn at(place: &Value) -> String {
    format!(
        r#"{{"position":{{"lat":{},"lng":{}}}}}"#,
        place["lat"], place["lng"]
    )
}

fn badge<'a>(discover: &'a Value, id: &str) -> &'a Value {
    discover["badges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == id)
        .unwrap()
}

#[tokio::test]
async fn health_reports_the_server_version() {
    let app = app().await;
    let (status, body) = call(&app, "GET", "/api/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert!(body.get("commit").is_some());
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
    assert_eq!(first["area"]["id"], "demo-leuven");
    assert_eq!(first["area"]["name"], "Leuven");
    assert_eq!(first["area"]["kind"], "city");
    // The second request is served from the cache.
    let (_, second) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    assert_eq!(first["countries"], second["countries"]);
    assert_eq!(first["area"]["fetched_at"], second["area"]["fetched_at"]);
}

#[tokio::test]
async fn a_postal_code_holds_part_of_its_city() {
    let app = app().await;
    let total = |d: &Value| {
        d["countries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["places"].as_u64().unwrap())
            .sum::<u64>()
    };
    let (_, city) = call(&app, "GET", "/api/discover?area=demo-leuven", None, None).await;
    let (status, west) = call(
        &app,
        "GET",
        "/api/discover?area=demo-leuven-3000",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(west["area"]["kind"], "postal_code");
    let (_, east) = call(
        &app,
        "GET",
        "/api/discover?area=demo-leuven-3010",
        None,
        None,
    )
    .await;
    assert!(total(&west) > 0 && total(&east) > 0);
    assert_eq!(total(&west) + total(&east), total(&city));
}

#[tokio::test]
async fn visiting_every_place_completes_a_country() {
    let app = app().await;
    let token = session(&app).await;
    let (_, discover) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    let country = discover["countries"][0]["code"]
        .as_str()
        .unwrap()
        .to_string();

    let (status, places) = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}&country={country}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let places = places.as_array().unwrap().clone();
    assert_eq!(
        places.len() as u64,
        discover["countries"][0]["places"].as_u64().unwrap()
    );
    assert!(places.iter().all(|p| p["countries"]
        .as_array()
        .unwrap()
        .contains(&Value::from(country.clone()))));
    // Open places first, then closed, then unknown hours; alphabetical within each group.
    let key = |p: &Value| {
        let group = match p["hours"]["status"]["open"].as_bool() {
            Some(true) => 0,
            Some(false) => 1,
            None => 2,
        };
        (group, p["name"].as_str().unwrap().to_lowercase())
    };
    assert!(places.windows(2).all(|w| key(&w[0]) <= key(&w[1])));
    assert!(places.iter().all(|p| p["visits"] == Value::Array(vec![])));

    // First check-in, made on the spot: 10 XP for the visit, 15 for being there, 50 for the country.
    let first = &places[0];
    let here = at(first);
    let (status, visit) = call(&app, "POST", &visits_uri(first), Some(&token), Some(&here)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(visit["visit"]["verified"], true);
    assert_eq!(visit["profile"]["xp"], 75);

    for place in &places[1..] {
        let uri = visits_uri(place);
        let (status, visit) = call(&app, "POST", &uri, Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(visit["visit"]["verified"], false);
        // Checking in twice changes nothing.
        call(&app, "POST", &uri, Some(&token), None).await;
    }

    let (_, after) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    let entry = &after["countries"][0];
    assert_eq!(entry["visited"], entry["places"]);
    assert_eq!(entry["status"], "completed");
    assert_eq!(after["world"]["discovered"], 1);
    assert_eq!(after["world"]["completed"], 1);
    assert_eq!(
        after["places_visited"].as_u64().unwrap(),
        places.len() as u64
    );
    assert_eq!(
        after["profile"]["visits"].as_u64().unwrap(),
        places.len() as u64
    );
    assert_eq!(badge(&after, "first-bite")["earned"], true);
    assert_eq!(badge(&after, "complete-1")["earned"], true);
    assert_eq!(badge(&after, "world")["earned"], false);

    // Another user sees none of it.
    let other = session(&app).await;
    let (_, theirs) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&other),
        None,
    )
    .await;
    assert_eq!(theirs["world"]["discovered"], 0);

    // Undoing a visit takes the completion away again.
    let uri = format!("/api/visits/{}", visit["visit"]["id"]);
    let (status, _) = call(&app, "DELETE", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, undone) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(undone["world"]["completed"], 0);
}

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
    assert_eq!(first["profile"]["xp"], 75);
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

    // Removing the latest visit leaves the place visited.
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/visits/{}", second["visit"]["id"]),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
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
    assert_eq!(status, StatusCode::NO_CONTENT);
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

#[tokio::test]
async fn rejects_bad_input() {
    let app = app().await;
    let (status, _) = call(&app, "GET", "/api/discover?area=Not%20An%20Id", None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&app, "GET", "/api/discover?area=demo-nowhere", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&app, "GET", "/api/areas/locate?lat=95&lng=4", None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&app, "POST", "/api/places/demo-x/visits", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some("nope"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let token = session(&app).await;
    let (status, body) = call(
        &app,
        "POST",
        "/api/places/does-not-exist/visits",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");
}

/// Asserts a JSON error body with a message and the given code.
fn assert_error(body: &Value, code: &str) {
    assert_eq!(body["code"], code, "{body}");
    assert!(
        !body["error"].as_str().unwrap_or_default().is_empty(),
        "{body}"
    );
}

#[tokio::test]
async fn every_error_is_json_with_a_code() {
    let app = app().await;
    let (status, body) = call(&app, "GET", "/api/discover?area=Not%20An%20Id", None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_error(&body, "bad_request");
    let (status, body) = call(&app, "GET", "/api/discover", Some("nope"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_error(&body, "unauthorized");

    // Rejected by axum's extractors before the handler runs.
    let (status, body) = call(&app, "GET", "/api/areas/locate?lng=4", None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_error(&body, "bad_request");
    assert!(body["error"].as_str().unwrap().contains("lat"), "{body}");
    let (status, body) = call(&app, "GET", "/api/areas/locate?lat=north&lng=4", None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_error(&body, "bad_request");
    let token = session(&app).await;
    let (status, body) = call(
        &app,
        "POST",
        "/api/places/demo-x/visits",
        Some(&token),
        Some("{not json"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_error(&body, "bad_request");
    let (status, body) = call(
        &app,
        "POST",
        "/api/push/subscription/delete",
        Some(&token),
        Some(r#"{"lat": 1}"#),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_error(&body, "bad_request");
    // A JSON body without the JSON content type.
    let request = Request::builder()
        .method("POST")
        .uri("/api/push/subscription/delete")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("{}"))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_error(&serde_json::from_slice(&bytes).unwrap(), "bad_request");

    let big = format!(r#"{{"lat": 1, "pad": "{}"}}"#, "x".repeat(3 << 20));
    let (status, body) = call(
        &app,
        "POST",
        "/api/places/demo-x/visits",
        Some(&token),
        Some(&big),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_error(&body, "bad_request");
    let (status, body) = call(&app, "POST", "/api/places/%FF/visits", Some(&token), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_error(&body, "bad_request");
    let (status, body) = call(
        &app,
        "DELETE",
        "/api/visits/not-a-number",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_error(&body, "bad_request");

    let (status, body) = call(&app, "POST", "/api/countries", None, None).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_error(&body, "method_not_allowed");
}

#[tokio::test]
async fn unknown_api_paths_are_a_json_404_even_next_to_the_web_app() {
    let dir = std::env::temp_dir().join(format!("mangiamundo-web-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("index.html"),
        "<!doctype html><title>web app</title>",
    )
    .unwrap();
    let app = api::with_web_app(app().await, &dir);

    for path in ["/api/nope", "/api/visits", "/api", "/api/"] {
        let (status, body) = call(&app, "GET", path, None, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_error(&body, "not_found");
    }
    let (status, body) = call(&app, "GET", "/api/countries", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["countries"].is_array());

    // Paths outside /api still reach the single-page app.
    let request = Request::builder()
        .uri("/passport")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&bytes).contains("web app"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn areas_can_be_searched_and_located() {
    let app = app().await;
    let (status, found) = call(&app, "GET", "/api/areas?q=leuv", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(found[0]["id"], "demo-leuven");
    assert_eq!(found[0]["kind"], "city");
    assert_eq!(found[0]["detail"], "Belgium");
    let (_, codes) = call(&app, "GET", "/api/areas?q=3010", None, None).await;
    assert_eq!(codes[0]["name"], "3010 Leuven");
    assert_eq!(codes[0]["kind"], "postal_code");
    let (status, suggested) = call(
        &app,
        "GET",
        "/api/areas/suggest?q=leu&lat=50.88&lng=4.7",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(suggested[0]["id"], "demo-leuven");
    let (_, here) = call(
        &app,
        "GET",
        "/api/areas/locate?lat=50.8798&lng=4.71",
        None,
        None,
    )
    .await;
    assert_eq!(here["city"]["id"], "demo-leuven");
    assert_eq!(here["postal_code"]["id"], "demo-leuven-3010");
    let (_, nowhere) = call(&app, "GET", "/api/areas/locate?lat=0&lng=0", None, None).await;
    assert!(nowhere["city"].is_null());
}

#[tokio::test]
async fn distances_are_measured_from_the_user_when_given() {
    let app = app().await;
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
    let uri = format!(
        "/api/places?{HERE}&country={country}&lat={}&lng={}",
        first["lat"], first["lng"]
    );
    let (_, from_there) = call(&app, "GET", &uri, None, None).await;
    let same = from_there
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == first["id"])
        .unwrap();
    assert_eq!(same["distance_m"], 0);
}

#[tokio::test]
async fn celebrations_only_list_countries_found_nearby() {
    let app = app().await;
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    let here: Vec<&str> = discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["code"].as_str().unwrap())
        .collect();

    let (status, list) = call(
        &app,
        "GET",
        &format!("/api/celebrations?{HERE}&days=366"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list = list.as_array().unwrap();
    assert!(!list.is_empty());
    let mut previous = String::new();
    for celebration in list {
        let starts = celebration["starts"].as_str().unwrap().to_string();
        assert!(starts >= previous, "sorted by date");
        assert!(celebration["ends"].as_str().unwrap() >= "2026-11-16");
        for nearby in celebration["nearby"].as_array().unwrap() {
            assert!(here.contains(&nearby["code"].as_str().unwrap()));
            assert!(nearby["places"].as_u64().unwrap() > 0);
        }
        assert!(!celebration["nearby"].as_array().unwrap().is_empty());
        assert!(celebration["reminder"]["title"]
            .as_str()
            .unwrap()
            .starts_with(celebration["title"].as_str().unwrap()));
        assert!(celebration["reminder"]["body"]
            .as_str()
            .unwrap()
            .contains("near you"));
        previous = starts;
    }
    // Thanksgiving is listed exactly when there is American food in the area.
    let thanksgiving = list.iter().any(|c| c["id"] == "thanksgiving");
    assert_eq!(thanksgiving, here.contains(&"US"));
}

#[tokio::test]
async fn events_move_from_upcoming_to_active_and_unearned_ones_disappear() {
    let (app, clock) = app_with_clock().await;
    let token = session(&app).await;
    // Japan month runs through December 2026.
    let (_, discover) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    let japan = discover["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "japan-month-2026-12")
        .unwrap();
    assert_eq!(japan["status"], "upcoming");
    assert_eq!(japan["progress"], 0);

    clock.store(START + 20 * DAY, Ordering::Relaxed); // 6 December
    let (_, during) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    let events = during["events"].as_array().unwrap();
    let levant = events.iter().find(|e| e["id"] == "levant-month-2026-11");
    assert!(
        levant.is_none(),
        "an unearned event disappears once it is over"
    );
    let japan = events
        .iter()
        .find(|e| e["id"] == "japan-month-2026-12")
        .unwrap();
    assert_eq!(japan["status"], "active");
    assert_eq!(japan["target"], 3);
}

#[tokio::test]
async fn event_days_follow_the_users_time_zone() {
    // 1 December 2026, 08:00 in Tokyo: Japan month has begun there, but it is still
    // 30 November (23:00) in UTC.
    let morning_in_tokyo = 1_796_079_600;
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (app, clock) = app_with(db, None, morning_in_tokyo).await;
    let token = session(&app).await;
    let (_, places) = call(
        &app,
        "GET",
        "/api/places?area=demo-tokyo&country=JP",
        Some(&token),
        None,
    )
    .await;
    let places = places.as_array().unwrap().clone();
    assert!(places.len() >= 2, "demo Tokyo has Japanese places");
    let visit = |i: usize, tz: &str| format!("{}{tz}", visits_uri(&places[i]));

    let japan = |discover: &Value| {
        discover["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == "japan-month-2026-12")
            .unwrap()
            .clone()
    };
    let (_, before) = call(
        &app,
        "GET",
        "/api/discover?area=demo-tokyo&tz=Asia%2FTokyo",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(japan(&before)["status"], "active");
    let (_, before) = call(
        &app,
        "GET",
        "/api/discover?area=demo-tokyo",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(
        japan(&before)["status"],
        "upcoming",
        "without a zone, today is the UTC date"
    );

    // Checked in on 1 December, Tokyo time: it counts, whatever zone asks later.
    let (status, _) = call(
        &app,
        "POST",
        &visit(0, "?tz=Asia%2FTokyo"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for tz in ["Asia%2FTokyo", "UTC", "America%2FLos_Angeles"] {
        let (_, now) = call(
            &app,
            "GET",
            &format!("/api/discover?area=demo-tokyo&tz={tz}"),
            Some(&token),
            None,
        )
        .await;
        assert_eq!(japan(&now)["progress"], 1, "progress seen from {tz}");
    }

    // 31 December 20:00 UTC, without a zone: dated 31 December, so it counts, even though
    // it was already 1 January in Tokyo (where the event, unearned, is now over).
    clock.store(1_798_747_200, Ordering::Relaxed);
    call(&app, "POST", &visit(1, ""), Some(&token), None).await;
    let (_, late) = call(
        &app,
        "GET",
        "/api/discover?area=demo-tokyo&tz=America%2FLos_Angeles",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(japan(&late)["progress"], 2);

    // An unknown zone is treated as UTC instead of failing.
    let (status, _) = call(
        &app,
        "GET",
        "/api/discover?area=demo-tokyo&tz=Etc%2FUnknown",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn celebrations_start_on_the_users_local_day() {
    use chrono::{NaiveDate, TimeZone};
    let tokyo: chrono_tz::Tz = "Asia/Tokyo".parse().unwrap();
    let app = app().await;
    // A one-day celebration with food in demo Tokyo, from 2027 so it is ahead of START.
    let (_, all) = call(
        &app,
        "GET",
        "/api/celebrations?area=demo-tokyo&days=366",
        None,
        None,
    )
    .await;
    let one_day = all
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["starts"] == c["ends"] && c["starts"].as_str().unwrap() >= "2027")
        .expect("a one-day celebration in demo Tokyo")
        .clone();
    let date: NaiveDate = one_day["starts"].as_str().unwrap().parse().unwrap();

    // 01:00 on that day in Tokyo, still the day before in UTC.
    let at = tokyo
        .from_local_datetime(&date.and_hms_opt(1, 0, 0).unwrap())
        .unwrap()
        .timestamp();
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (app, _) = app_with(db, None, at).await;
    let ids = |list: &Value| {
        list.as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].clone())
            .collect::<Vec<_>>()
    };
    let (_, local) = call(
        &app,
        "GET",
        "/api/celebrations?area=demo-tokyo&days=0&tz=Asia%2FTokyo",
        None,
        None,
    )
    .await;
    assert!(ids(&local).contains(&one_day["id"]), "it is today in Tokyo");
    let (_, utc) = call(
        &app,
        "GET",
        "/api/celebrations?area=demo-tokyo&days=0",
        None,
        None,
    )
    .await;
    assert!(!ids(&utc).contains(&one_day["id"]), "it is tomorrow in UTC");
}

#[tokio::test]
async fn visits_between_counts_distinct_places_inside_the_window() {
    use mangiamundo::geo::{Bbox, Shape};
    use mangiamundo::providers::{AreaInfo, AreaKind, Boundary, FoundPlace};
    use mangiamundo::store::{save_area, Area};

    let db = store::connect("sqlite::memory:").await.unwrap();
    let place = |id: &str, countries: Vec<&'static str>| FoundPlace {
        id: id.into(),
        name: id.into(),
        lat: 50.0,
        lng: 4.0,
        address: None,
        url: None,
        cuisine: None,
        opening_hours: None,
        countries,
    };
    let info = AreaInfo {
        id: "test-area".into(),
        name: "Test".into(),
        kind: AreaKind::City,
        detail: None,
        lat: 50.0,
        lng: 4.0,
    };
    let shape = Shape::rect(Bbox {
        south: 49.9,
        west: 3.9,
        north: 50.1,
        east: 4.1,
    });
    let area = Area {
        boundary: Boundary { info, shape },
        provider: "test".into(),
        fetched_at: START,
    };
    save_area(
        &db,
        &area,
        &[
            place("sushi", vec!["JP"]),
            place("ramen", vec!["JP"]),
            place("fusion", vec!["JP", "KR"]),
            place("pasta", vec!["IT"]),
        ],
    )
    .await
    .unwrap();
    let token = store::create_user(&db).await.unwrap();
    let user = store::user_for_token(&db, &token).await.unwrap().unwrap();

    let day = |s: &str| s.parse::<chrono::NaiveDate>().unwrap();
    let (first, last) = (day("2026-12-01"), day("2026-12-31"));
    // The stored local date decides, not the instant.
    store::add_visit(&db, &user, "sushi", false, START, day("2026-11-30"))
        .await
        .unwrap(); // the day before
    store::add_visit(&db, &user, "ramen", false, START, first)
        .await
        .unwrap(); // first day
    store::add_visit(&db, &user, "fusion", false, START, last)
        .await
        .unwrap(); // last day, two countries
    store::add_visit(&db, &user, "pasta", false, START, day("2026-12-02"))
        .await
        .unwrap(); // wrong country

    let japan = vec!["JP".to_string()];
    assert_eq!(
        store::visits_between(&db, &user, &japan, first, last)
            .await
            .unwrap(),
        2
    );
    let east_asia = vec!["JP".to_string(), "KR".to_string()];
    assert_eq!(
        store::visits_between(&db, &user, &east_asia, first, last)
            .await
            .unwrap(),
        2,
        "a place counts once"
    );
    assert_eq!(
        store::visits_between(&db, &user, &[], first, last)
            .await
            .unwrap(),
        0
    );
}

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

    assert_eq!(
        call(&app, "PUT", &report, Some(&alice), None).await.0,
        StatusCode::NO_CONTENT
    );
    // Reporting twice counts once.
    assert_eq!(
        call(&app, "PUT", &report, Some(&alice), None).await.0,
        StatusCode::NO_CONTENT
    );
    assert!(!list(Some(alice.clone())).await.contains(&place));
    assert!(list(Some(bob.clone())).await.contains(&place));
    assert!(list(None).await.contains(&place));

    call(&app, "PUT", &report, Some(&bob), None).await;
    assert!(!list(None).await.contains(&place));
}

/// A fake OSM Notes API: its URL, and the notes posted to it as `<authorization> <form
/// body>`, with `-` for an anonymous note. Each note gets id 7.
async fn fake_osm_notes() -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
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
    use mangiamundo::geo::{Bbox, Shape};
    use mangiamundo::providers::{AreaInfo, AreaKind, Boundary};
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
    let (app, _) = app_with(db, Some(url), START).await;

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
            StatusCode::NO_CONTENT
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

#[tokio::test]
async fn an_added_place_shows_up_for_whoever_added_it_and_is_sent_to_osm() {
    let (url, notes) = fake_osm_notes().await;
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (app, clock) = app_with_notes(db.clone(), Some(url), Some("secret".into()), START).await;
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

/// Writes real responses to `contract/`, where the Android and iOS tests decode them.
/// CI fails when the checked-in files differ, so an API change shows up in the diff.
#[tokio::test]
async fn a_check_in_says_what_it_unlocked_in_the_area() {
    let app = app().await;
    let token = session(&app).await;
    let discover = |token: String| {
        let app = &app;
        async move {
            let uri = format!("/api/discover?{HERE}");
            call(app, "GET", &uri, Some(&token), None).await.1
        }
    };
    let before = discover(token.clone()).await;
    let busy = before["countries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["places"].as_u64().unwrap() >= 3)
        .unwrap();
    let country = busy["code"].as_str().unwrap();
    let uri = format!("/api/places?{HERE}&country={country}");
    let (_, places) = call(&app, "GET", &uri, Some(&token), None).await;
    let (first, second) = (&places[0], &places[1]);
    let in_area = |place: &Value| format!("{}?{HERE}", visits_uri(place));

    // An area that doesn't exist: an error, and no visit is kept.
    let uri = format!("{}?area=demo-nowhere", visits_uri(first));
    let (status, _) = call(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(discover(token.clone()).await["places_visited"], 0);

    // The first check-in: the country turns gold and the first badge is earned.
    let (status, result) = call(&app, "POST", &in_area(first), Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let unlocked = result["unlocked"].as_array().unwrap();
    assert_eq!(unlocked[0]["kind"], "country");
    assert_eq!(unlocked[0]["country"], country);
    assert!(unlocked[0]["title"].as_str().unwrap().ends_with(": gold"));
    let more = busy["goal"].as_u64().unwrap() - 1;
    assert_eq!(unlocked[0]["text"], format!("{more} more for diamond."));
    let first_bite = badge(&before, "first-bite");
    let earned = unlocked.iter().find(|u| u["kind"] == "badge").unwrap();
    assert_eq!(earned["badge"], first_bite["kind"]);
    assert_eq!(
        earned["title"],
        format!("Badge earned: {}", first_bite["title"].as_str().unwrap())
    );
    assert_eq!(earned["text"], first_bite["description"]);
    // The progress is what discover says next, so clients need not ask.
    let after = discover(token.clone()).await;
    for part in ["countries", "badges", "profile", "events", "quests", "world"] {
        assert_eq!(result["progress"][part], after[part], "{part}");
    }
    assert_eq!(result["profile"], after["profile"]);

    // Again the same day, or at another place of a country that stays gold: nothing new
    // for the country.
    let (_, again) = call(&app, "POST", &in_area(first), Some(&token), None).await;
    assert_eq!(again["unlocked"], serde_json::json!([]));
    let (_, next) = call(&app, "POST", &in_area(second), Some(&token), None).await;
    assert!(next["unlocked"]
        .as_array()
        .unwrap()
        .iter()
        .all(|u| u["kind"] != "country"));

    // Without an area there is nothing to compare.
    let other = session(&app).await;
    let (_, plain) = call(&app, "POST", &visits_uri(first), Some(&other), None).await;
    assert_eq!(plain["progress"], Value::Null);
    assert_eq!(plain["unlocked"], serde_json::json!([]));
}

#[tokio::test]
async fn writes_contract_fixtures() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../contract");
    let write = |name: &str, value: &Value| {
        std::fs::write(
            dir.join(format!("{name}.json")),
            serde_json::to_string_pretty(value).unwrap() + "\n",
        )
        .unwrap();
    };
    let db = store::connect("sqlite::memory:").await.unwrap();
    let unknown = store::UnknownPlace {
        id: "osm-node-1".into(),
        name: "Lalibela".into(),
        lat: 50.879,
        lng: 4.701,
        address: Some("Naamsestraat 12, Leuven".into()),
        distance_m: 0,
    };
    store::replace_unknown(&db, &[unknown]).await.unwrap();
    let (app, _) = app_with(db, None, START).await;
    let token = session(&app).await;
    let get = |uri: String| {
        let (app, token) = (&app, &token);
        async move {
            let (status, body) = call(app, "GET", &uri, Some(token), None).await;
            assert_eq!(status, StatusCode::OK, "{uri}");
            body
        }
    };

    // One verified and one unverified check-in, so progress, badges and places show both.
    // At least 3 places, so the two visits taste the country without completing it.
    let discover = get(format!("/api/discover?{HERE}")).await;
    let busy = discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["places"].as_u64().unwrap() >= 3)
        .unwrap();
    let country = busy["code"].as_str().unwrap().to_string();
    let places = get(format!("/api/places?{HERE}&country={country}")).await;
    let (first, second) = (&places[0], &places[1]);
    let (status, visit) = call(
        &app,
        "POST",
        &format!("{}?{HERE}", visits_uri(first)),
        Some(&token),
        Some(&at(first)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    call(&app, "POST", &visits_uri(second), Some(&token), None).await;

    let discover = get(format!("/api/discover?{HERE}")).await;
    write("countries", &get("/api/countries".into()).await);
    write("discover", &discover);
    write(
        "places",
        &get(format!("/api/places?{HERE}&country={country}")).await,
    );
    write(
        "celebrations",
        &get(format!("/api/celebrations?{HERE}&days=90")).await,
    );
    write(
        "reminders",
        &get(format!("/api/reminders?{HERE}&days=90")).await,
    );
    write("visit", &visit);
    write(
        "place",
        &get(format!(
            "/api/places/{}?lat=50.8798&lng=4.7005",
            first["id"].as_str().unwrap()
        ))
        .await,
    );
    write("areas", &get("/api/areas?q=leuv".into()).await);
    write(
        "unknown-places",
        &get(format!("/api/unknown-places?{HERE}&lat=50.8798&lng=4.7005")).await,
    );
    write(
        "located",
        &get("/api/areas/locate?lat=50.8798&lng=4.71".into()).await,
    );
    let (status, error) = call(&app, "GET", "/api/discover?area=demo-nowhere", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    write("error", &error);
    let new = r#"{"name":"Kiwi","country":"NZ","lat":50.8798,"lng":4.7005,"note":false}"#;
    let (status, added) = call(&app, "POST", "/api/places", Some(&token), Some(new)).await;
    assert_eq!(status, StatusCode::OK);
    // Its id is random.
    let mut added = added;
    added["id"] = "user-0123456789abcdef0123456789abcdef".into();
    write("added", &added);
    let mut additions = get("/api/additions".into()).await;
    additions[0]["place_id"] = added["id"].clone();
    additions[0]["id"] = added["id"].clone();
    write("additions", &additions);

    // Two devices, one with a name, and a recovery code. Ids and times are random or real.
    call(
        &app,
        "POST",
        "/api/account/recovery-code",
        Some(&token),
        None,
    )
    .await;
    let (_, link) = call(&app, "POST", "/api/account/link-code", Some(&token), None).await;
    let body = format!(
        r#"{{"code":"{}","device":"iPhone"}}"#,
        link["code"].as_str().unwrap()
    );
    assert_eq!(
        call(&app, "POST", "/api/session/restore", None, Some(&body))
            .await
            .0,
        StatusCode::OK
    );
    let mut account = get("/api/account".into()).await;
    account["created_at"] = START.into();
    account["recovery_code_created_at"] = (START + 60).into();
    for (i, device) in account["devices"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        device["id"] = format!("{i}0123456789abcdef0123456789abcde").into();
        device["created_at"] = START.into();
        device["last_used_at"] = (START + 120).into();
    }
    write("account", &account);
    write(
        "link-code",
        &serde_json::json!({ "code": "ABCD-EFGH", "expires_at": START + 600 }),
    );

    // What a newer server might send: enum values this app version doesn't know.
    let mut newer = discover.clone();
    newer["badges"][0]["kind"] = "something-new".into();
    newer["countries"][0]["status"] = "ruby".into();
    newer["events"][0]["status"] = "paused".into();
    newer["unknown_field"] = true.into();
    write("discover-newer", &newer);
}

#[tokio::test]
async fn an_area_that_is_not_loading_has_no_progress() {
    let response = app()
        .await
        .oneshot(
            Request::get("/api/areas/demo-leuven/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"null");
}

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
        assert!(s["place"]["visits"].as_array().unwrap().is_empty());
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

/// Writes the OpenAPI document to `contract/`, so CI shows API changes in the diff.
/// It must match what the server serves.
#[tokio::test]
async fn serves_and_writes_the_openapi_document() {
    let app = app().await;
    let (status, doc) = call(&app, "GET", "/api/openapi.json", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(doc["openapi"].as_str().unwrap().get(..3), Some("3.1"));
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../contract/openapi.json");
    std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}

#[tokio::test]
async fn an_added_place_earns_xp_once_the_import_confirms_it() {
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (app, clock) = app_with(db.clone(), None, START).await;
    let (alice, bob, carol) = (
        session(&app).await,
        session(&app).await,
        session(&app).await,
    );

    // Countries by what they would bring to Leuven: one already there, one new from a
    // continent that is there, and two new from a continent that isn't.
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    let here: Vec<&str> = discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["code"].as_str().unwrap())
        .collect();
    let continents_here: Vec<&str> = discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["continent"].as_str().unwrap())
        .collect();
    let (_, catalog) = call(&app, "GET", "/api/countries", None, None).await;
    let addable: Vec<&Value> = catalog["countries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| !c["cuisine"].is_null())
        .collect();
    let code = |c: &&Value| c["code"].as_str().unwrap().to_string();
    let present = addable
        .iter()
        .find(|c| here.contains(&c["code"].as_str().unwrap()))
        .map(code)
        .unwrap();
    let new_country = addable
        .iter()
        .find(|c| {
            !here.contains(&c["code"].as_str().unwrap())
                && continents_here.contains(&c["continent"].as_str().unwrap())
        })
        .map(code)
        .unwrap();
    let elsewhere: Vec<&&Value> = addable
        .iter()
        .filter(|c| !continents_here.contains(&c["continent"].as_str().unwrap()))
        .collect();
    let far = elsewhere
        .iter()
        .find(|c| {
            elsewhere
                .iter()
                .any(|o| o["continent"] == c["continent"] && o["code"] != c["code"])
        })
        .unwrap();
    let (far_a, far_b) = (
        code(far),
        elsewhere
            .iter()
            .find(|o| o["continent"] == far["continent"] && o["code"] != far["code"])
            .map(|c| code(c))
            .unwrap(),
    );

    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}"), None, None).await;
    let (lat, lng) = (
        places[0]["lat"].as_f64().unwrap(),
        places[0]["lng"].as_f64().unwrap(),
    );
    let add = |token: String, name: &'static str, country: String| {
        let (app, clock) = (app.clone(), clock.clone());
        async move {
            clock.fetch_add(60, Ordering::Relaxed);
            let body = format!(
                r#"{{"name":"{name}","country":"{country}","lat":{lat},"lng":{lng},"note":false}}"#
            );
            let (status, place) =
                call(&app, "POST", "/api/places", Some(&token), Some(&body)).await;
            assert_eq!(status, StatusCode::OK, "{place}");
            place
        }
    };
    let reward = |place: &Value| {
        (
            place["reward"]["bonus"].as_str().unwrap().to_string(),
            place["reward"]["xp"].as_u64().unwrap(),
        )
    };
    let s = |v: &str| v.to_string();

    // Decided when added, ignoring other pending additions.
    let alpha = add(alice.clone(), "Alpha", far_a.clone()).await;
    assert_eq!(reward(&alpha), (s("continent"), 380));
    assert_eq!(alpha["reward"]["area_name"], "Leuven");
    assert_eq!(
        alpha["countries"],
        serde_json::json!([far_a]),
        "still a place"
    );
    assert_eq!(
        reward(&add(bob.clone(), "Beta", far_b.clone()).await),
        (s("continent"), 380)
    );
    assert_eq!(
        reward(&add(bob.clone(), "alpha", far_a.clone()).await),
        (s("continent"), 380)
    );
    assert_eq!(
        reward(&add(carol.clone(), "Gamma", present.clone()).await),
        (s("none"), 30)
    );
    assert_eq!(
        reward(&add(carol.clone(), "Delta", new_country.clone()).await),
        (s("country"), 130)
    );

    let get = |uri: &'static str, token: String| {
        let app = app.clone();
        async move { call(&app, "GET", uri, Some(&token), None).await.1 }
    };
    let xp = |token: String| {
        let app = app.clone();
        async move {
            call(
                &app,
                "GET",
                &format!("/api/discover?{HERE}"),
                Some(&token),
                None,
            )
            .await
            .1["profile"]["xp"]
                .as_u64()
                .unwrap()
        }
    };
    let pending = get("/api/additions", alice.clone()).await;
    assert_eq!(pending[0]["status"], "pending");
    assert_eq!(pending[0]["xp"], 380);
    assert_eq!(xp(alice.clone()).await, 0, "nothing before confirmation");
    assert_eq!(
        call(&app, "GET", "/api/additions", None, None).await.0,
        StatusCode::UNAUTHORIZED
    );

    // The import brings all four restaurants (Alpha once).
    let twin = |n: u32, name: &str, country: &str| {
        let cuisine = mangiamundo::catalog::cuisine_tag(country).unwrap();
        mangiamundo::providers::FoundPlace {
            id: format!("osm-node-{n}"),
            name: name.into(),
            lat,
            lng,
            address: None,
            url: None,
            cuisine: Some(cuisine.into()),
            opening_hours: None,
            countries: vec![],
        }
    };
    let all = mangiamundo::geo::Bbox {
        south: -90.0,
        west: -180.0,
        north: 90.0,
        east: 180.0,
    };
    let import = [
        twin(1, "Alpha", &far_a),
        twin(2, "Beta", &far_b),
        twin(3, "Gamma", &present),
        twin(4, "Delta", &new_country),
    ];
    assert_eq!(store::replace_imported(&db, all, &import).await.unwrap(), 5);

    let mine = get("/api/additions", alice.clone()).await;
    assert_eq!(mine[0]["status"], "confirmed");
    assert_eq!(mine[0]["place_id"], "osm-node-1");
    assert_eq!(mine[0]["seen"], false);
    assert_eq!(xp(alice.clone()).await, 380);
    let (_, discover) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(badge(&discover, "pioneer")["earned"], true);
    assert_eq!(discover["profile"]["added"], 1);

    // Bob's continent went to Alice, who was first, but Beta still brings a new country.
    // His Alpha was hers already.
    let his = get("/api/additions", bob.clone()).await;
    let status = |a: &Value| {
        (
            a["name"].as_str().unwrap().to_string(),
            a["status"].as_str().unwrap().to_string(),
            a["bonus"].as_str().unwrap().to_string(),
            a["xp"].as_u64().unwrap(),
        )
    };
    assert_eq!(status(&his[0]), (s("alpha"), s("duplicate"), s("none"), 0));
    assert_eq!(
        status(&his[1]),
        (s("Beta"), s("confirmed"), s("country"), 130)
    );
    assert_eq!(xp(bob.clone()).await, 130);
    assert_eq!(xp(carol.clone()).await, 30 + 130);

    // Only what the caller lists, and only their own.
    let seen = |ids: &[&Value]| format!(r#"{{"ids":{}}}"#, serde_json::to_string(ids).unwrap());
    let put = |token: String, body: String| {
        let app = app.clone();
        async move {
            call(
                &app,
                "PUT",
                "/api/additions/seen",
                Some(&token),
                Some(&body),
            )
            .await
            .0
        }
    };
    let beta = &his[1]["id"];
    assert_eq!(
        put(alice.clone(), seen(&[beta])).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        get("/api/additions", bob.clone()).await[1]["seen"],
        false,
        "Beta is Bob's"
    );
    assert_eq!(
        get("/api/additions", alice.clone()).await[0]["seen"],
        false,
        "not listed"
    );
    assert_eq!(
        put(alice.clone(), seen(&[&mine[0]["id"]])).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(get("/api/additions", alice.clone()).await[0]["seen"], true);

    let too_many: Vec<Value> = (0..101).map(|i| Value::from(format!("user-{i}"))).collect();
    assert_eq!(
        put(bob.clone(), seen(&too_many.iter().collect::<Vec<_>>())).await,
        StatusCode::BAD_REQUEST
    );
}

/// A provider whose OpenStreetMap answers the test sets: areas come from the demo provider.
#[derive(Default)]
struct FakeOsm {
    places: std::sync::Mutex<std::collections::HashMap<String, mangiamundo::providers::Lookup>>,
    nearby: std::sync::Mutex<Vec<mangiamundo::providers::FoundPlace>>,
    down: std::sync::atomic::AtomicBool,
}

#[async_trait::async_trait]
impl mangiamundo::providers::PlacesProvider for FakeOsm {
    fn name(&self) -> &'static str {
        "osm"
    }
    async fn search(
        &self,
        area: &mangiamundo::providers::Boundary,
        next: &(dyn Fn() + Send + Sync),
    ) -> anyhow::Result<Vec<mangiamundo::providers::FoundPlace>> {
        mangiamundo::providers::PlacesProvider::search(&Demo, area, next).await
    }
    async fn place(&self, id: &str) -> anyhow::Result<mangiamundo::providers::Lookup> {
        anyhow::ensure!(!self.down.load(Ordering::Relaxed), "overpass is down");
        Ok(self
            .places
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .unwrap_or(mangiamundo::providers::Lookup::Gone))
    }
    async fn nearby(
        &self,
        _lat: f64,
        _lng: f64,
        _radius_m: f64,
    ) -> anyhow::Result<Vec<mangiamundo::providers::FoundPlace>> {
        anyhow::ensure!(!self.down.load(Ordering::Relaxed), "overpass is down");
        Ok(self.nearby.lock().unwrap().clone())
    }
}

/// An app on `osm`, with Leuven cached holding one OSM place, osm-node-42 (Lalibela, Ethiopian).
async fn app_with_osm() -> (Router, Arc<AtomicI64>, Arc<FakeOsm>, sqlx::SqlitePool) {
    use mangiamundo::geo::{Bbox, Shape};
    use mangiamundo::providers::{AreaInfo, AreaKind, Boundary};
    let db = store::connect("sqlite::memory:").await.unwrap();
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
        provider: "osm".into(),
        fetched_at: START,
    };
    store::save_area(
        &db,
        &area,
        &[osm_place(42, "Lalibela", "ethiopian", 50.8798, 4.7005)],
    )
    .await
    .unwrap();
    let fake = Arc::new(FakeOsm::default());
    let now = Arc::new(AtomicI64::new(START));
    let clock = now.clone();
    let router = api::router(AppState {
        loads: Default::default(),
        limits: Default::default(),
        db: db.clone(),
        places: fake.clone(),
        areas: Arc::new(Demo),
        cache_ttl_secs: 3600,
        clock: Arc::new(move || clock.load(Ordering::Relaxed)),
        push_key: None,
        osm_notes_url: None,
        osm_notes_token: None,
        auth: Default::default(),
    });
    (router, now, fake, db)
}

fn osm_place(
    n: u64,
    name: &str,
    cuisine: &str,
    lat: f64,
    lng: f64,
) -> mangiamundo::providers::FoundPlace {
    mangiamundo::providers::FoundPlace {
        id: format!("osm-node-{n}"),
        name: name.into(),
        lat,
        lng,
        address: None,
        url: None,
        cuisine: Some(cuisine.into()),
        opening_hours: None,
        countries: mangiamundo::catalog::countries_for_cuisine_tag(cuisine),
    }
}

async fn countries_here(app: &Router, token: &str) -> Vec<String> {
    let (_, discover) = call(
        app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(token),
        None,
    )
    .await;
    discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["code"].as_str().unwrap().to_string())
        .collect()
}

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

#[tokio::test]
async fn a_recovery_code_restores_the_account_on_a_new_device() {
    let app = app().await;
    let old = session(&app).await;
    let places = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}"),
        Some(&old),
        None,
    )
    .await
    .1;
    let (first, second) = (&places[0], &places[1]);
    assert_eq!(
        call(&app, "POST", &visits_uri(first), Some(&old), None)
            .await
            .0,
        StatusCode::OK
    );

    let (status, account) = call(&app, "GET", "/api/account", Some(&old), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(account["recovery_code_created_at"].is_null());
    assert_eq!(account["devices"].as_array().unwrap().len(), 1);
    assert_eq!(account["devices"][0]["current"], true);

    let (status, body) = call(&app, "POST", "/api/account/recovery-code", Some(&old), None).await;
    assert_eq!(status, StatusCode::OK);
    let code = body["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 24);

    // A new phone already checked in once before restoring: that visit is kept.
    let new = session(&app).await;
    assert_eq!(
        call(&app, "POST", &visits_uri(second), Some(&new), None)
            .await
            .0,
        StatusCode::OK
    );
    let typed = code.to_lowercase().replace('-', " ");
    let body = format!(r#"{{"code":"{typed}","device":"iPhone"}}"#);
    let (status, restored) = call(
        &app,
        "POST",
        "/api/session/restore",
        Some(&new),
        Some(&body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let restored = restored["token"].as_str().unwrap().to_string();

    // The new phone's anonymous session is gone, its visit moved over.
    assert_eq!(
        call(&app, "GET", "/api/account", Some(&new), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (_, profile) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&restored),
        None,
    )
    .await;
    assert_eq!(profile["profile"]["visits"], 2, "{profile}");

    let (_, account) = call(&app, "GET", "/api/account", Some(&restored), None).await;
    let devices = account["devices"].as_array().unwrap();
    assert_eq!(devices.len(), 2);
    assert!(account["recovery_code_created_at"].is_i64());
    let current = devices.iter().find(|d| d["current"] == true).unwrap();
    assert_eq!(current["name"], "iPhone");

    // The code keeps working until a new one replaces it.
    let again = format!(r#"{{"code":"{code}"}}"#);
    assert_eq!(
        call(&app, "POST", "/api/session/restore", None, Some(&again))
            .await
            .0,
        StatusCode::OK
    );
    call(&app, "POST", "/api/account/recovery-code", Some(&old), None).await;
    let (status, error) = call(&app, "POST", "/api/session/restore", None, Some(&again)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_error(&error, "not_found");
}

#[tokio::test]
async fn a_link_code_signs_in_one_device_once_within_ten_minutes() {
    let (app, clock) = app_with_clock().await;
    let phone = session(&app).await;
    let (status, link) = call(&app, "POST", "/api/account/link-code", Some(&phone), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(link["expires_at"], START + 600);
    let body = format!(
        r#"{{"code":"{}","device":"Web"}}"#,
        link["code"].as_str().unwrap()
    );

    let (status, web) = call(&app, "POST", "/api/session/restore", None, Some(&body)).await;
    assert_eq!(status, StatusCode::OK);
    let web = web["token"].as_str().unwrap().to_string();
    assert_eq!(
        call(&app, "POST", "/api/session/restore", None, Some(&body))
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    let (_, account) = call(&app, "GET", "/api/account", Some(&web), None).await;
    assert_eq!(account["devices"].as_array().unwrap().len(), 2);

    let (_, link) = call(&app, "POST", "/api/account/link-code", Some(&phone), None).await;
    let body = format!(r#"{{"code":"{}"}}"#, link["code"].as_str().unwrap());
    clock.store(START + 601, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        call(&app, "POST", "/api/session/restore", None, Some(&body))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn devices_can_be_signed_out_and_the_account_deleted() {
    let app = app().await;
    let phone = session(&app).await;
    let (_, link) = call(&app, "POST", "/api/account/link-code", Some(&phone), None).await;
    let body = format!(r#"{{"code":"{}"}}"#, link["code"].as_str().unwrap());
    let laptop = call(&app, "POST", "/api/session/restore", None, Some(&body))
        .await
        .1["token"]
        .as_str()
        .unwrap()
        .to_string();

    let (_, account) = call(&app, "GET", "/api/account", Some(&phone), None).await;
    let other = account["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["current"] == false)
        .unwrap();
    let uri = format!("/api/account/devices/{}", other["id"].as_str().unwrap());
    // Someone else can't sign a device of this account out.
    let stranger = session(&app).await;
    assert_eq!(
        call(&app, "DELETE", &uri, Some(&stranger), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, "DELETE", &uri, Some(&phone), None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "GET", "/api/account", Some(&laptop), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        call(&app, "DELETE", "/api/account", Some(&phone), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "GET", "/api/account", Some(&phone), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/api/account", Some(&stranger), None)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn link_codes_are_limited_per_hour() {
    let app = app().await;
    let token = session(&app).await;
    for _ in 0..10 {
        assert_eq!(
            call(&app, "POST", "/api/account/link-code", Some(&token), None)
                .await
                .0,
            StatusCode::OK
        );
    }
    let (status, error) = call(&app, "POST", "/api/account/link-code", Some(&token), None).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_error(&error, "too_many_requests");
}

/// A restore that read its session before another restore moved that session to a new
/// account merges that new account, not the stale one.
#[tokio::test]
async fn a_restore_merges_the_account_its_session_belongs_to_now() {
    use mangiamundo::account;
    let db = store::connect("sqlite::memory:").await.unwrap();
    let user = |token: String| {
        let db = db.clone();
        async move {
            account::session_for_token(&db, &token, START)
                .await
                .unwrap()
                .unwrap()
        }
    };
    let stale = user(account::create_user(&db, None, START).await.unwrap()).await;
    let moved_to = user(account::create_user(&db, None, START).await.unwrap())
        .await
        .user;
    let target = user(account::create_user(&db, None, START).await.unwrap())
        .await
        .user;
    // The other restore, done meanwhile: this session now belongs to `moved_to`.
    sqlx::query("UPDATE sessions SET user_id = ? WHERE id = ?")
        .bind(&moved_to)
        .bind(&stale.id)
        .execute(&db)
        .await
        .unwrap();
    let code = account::new_recovery_code(&db, &target, START)
        .await
        .unwrap();
    account::redeem(&db, &code, Some(&stale), None, START)
        .await
        .unwrap()
        .unwrap();
    let left: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE id = ?")
        .bind(&moved_to)
        .fetch_optional(&db)
        .await
        .unwrap();
    assert_eq!(
        left, None,
        "`moved_to` was merged into the restored account"
    );
}

/// A GET of a public page or image: its status, content type and bytes.
async fn fetch(app: &Router, uri: &str) -> (StatusCode, String, Vec<u8>) {
    let request = Request::builder().uri(uri).header("host", "mangiamundo.test").body(Body::empty()).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let kind = response
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let bytes = response.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, kind, bytes)
}

/// The path of a shared link: `https://mangiamundo.test/p/abc` → `/p/abc`.
fn path_of(url: &Value) -> String {
    let url = url.as_str().unwrap();
    assert!(url.starts_with("https://mangiamundo.test/"), "{url}");
    url.trim_start_matches("https://mangiamundo.test").to_string()
}

async fn post_shared(app: &Router, uri: &str, token: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("host", "mangiamundo.test")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn sharing_progress_makes_a_public_page_that_can_be_turned_off() {
    let app = app().await;
    let token = session(&app).await;
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), Some(&token), None).await;
    let country = discover["countries"][0]["code"].as_str().unwrap().to_string();
    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}&country={country}"), Some(&token), None).await;
    let (status, _) = call(&app, "POST", &visits_uri(&places[0]), Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, link) = post_shared(&app, &format!("/api/share?{HERE}"), &token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(link["text"].as_str().unwrap().contains("I've tasted food from 1 of the"));
    let page = path_of(&link["url"]);
    assert_eq!(path_of(&link["image_url"]), format!("{page}/card.png"));

    // Sharing again keeps the link.
    let (_, again) = post_shared(&app, &format!("/api/share?{HERE}"), &token).await;
    assert_eq!(again["url"], link["url"]);
    let (_, account) = call(&app, "GET", "/api/account", Some(&token), None).await;
    assert_eq!(account["public_profile"].as_str(), Some(page.as_str()));

    // Anyone can open the page and its card, which shows no places.
    let (status, kind, html) = fetch(&app, &page).await;
    assert_eq!(status, StatusCode::OK);
    assert!(kind.starts_with("text/html"));
    let html = String::from_utf8(html).unwrap();
    assert!(html.contains(r#"<meta property="og:image" content="https://mangiamundo.test/p/"#));
    assert!(html.contains("1 of "));
    assert!(!html.contains(places[0]["name"].as_str().unwrap()), "no place names on the public page");
    let (status, kind, png) = fetch(&app, &format!("{page}/card.png")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(kind, "image/png");
    assert!(png.starts_with(b"\x89PNG"));

    // Turned off, the link is gone.
    let (status, _) = call(&app, "DELETE", "/api/share", Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(fetch(&app, &page).await.0, StatusCode::NOT_FOUND);
    let (_, account) = call(&app, "GET", "/api/account", Some(&token), None).await;
    assert!(account["public_profile"].is_null());
}

#[tokio::test]
async fn a_shared_check_in_has_its_own_page_until_it_is_removed() {
    let app = app().await;
    let token = session(&app).await;
    let other = session(&app).await;
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), Some(&token), None).await;
    let country = discover["countries"][0]["code"].as_str().unwrap().to_string();
    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}&country={country}"), Some(&token), None).await;
    let (_, result) = call(&app, "POST", &visits_uri(&places[0]), Some(&token), None).await;
    let visit = result["visit"]["id"].as_i64().unwrap();

    // Only its owner can share it.
    let (status, _) = post_shared(&app, &format!("/api/visits/{visit}/share"), &other).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, link) = post_shared(&app, &format!("/api/visits/{visit}/share"), &token).await;
    assert_eq!(status, StatusCode::OK);
    let name = places[0]["name"].as_str().unwrap();
    assert!(link["text"].as_str().unwrap().contains(name));
    let (_, again) = post_shared(&app, &format!("/api/visits/{visit}/share"), &token).await;
    assert_eq!(again["url"], link["url"], "the same link each time");

    let page = path_of(&link["url"]);
    let (status, _, html) = fetch(&app, &page).await;
    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8(html).unwrap().contains(name));
    let (status, kind, _) = fetch(&app, &format!("{page}/card.png")).await;
    assert_eq!((status, kind.as_str()), (StatusCode::OK, "image/png"));

    let (status, _) = call(&app, "DELETE", &format!("/api/visits/{visit}"), Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(fetch(&app, &page).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn monthly_quests_pay_once_and_the_streak_shows() {
    let app = app().await;
    let token = session(&app).await;
    let discover = |app: Router, token: String| async move {
        call(
            &app,
            "GET",
            &format!("/api/discover?{HERE}"),
            Some(&token),
            None,
        )
        .await
        .1
    };
    let before = discover(app.clone(), token.clone()).await;
    let check_ins = |d: &Value| {
        d["quests"]["quests"]
            .as_array()
            .unwrap()
            .iter()
            .find(|q| q["id"] == "check-ins")
            .cloned()
            .expect("check-ins quest")
    };
    assert_eq!(before["quests"]["month"], "2026-11");
    assert_eq!(check_ins(&before)["done"], false);
    assert_eq!(before["profile"]["streak"], 0);

    let (_, places) = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    for place in &places.as_array().unwrap()[..2] {
        call(&app, "POST", &visits_uri(place), Some(&token), None).await;
    }
    let after = discover(app.clone(), token.clone()).await;
    assert_eq!(check_ins(&after)["done"], true);
    assert_eq!(after["profile"]["streak"], 1);
    let xp = after["profile"]["xp"].as_u64().unwrap();
    // Asking again doesn't pay the quest twice.
    let again = discover(app, token).await;
    assert_eq!(again["profile"]["xp"].as_u64().unwrap(), xp);
    let quest_xp: u64 = after["quests"]["quests"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|q| q["done"] == true)
        .map(|q| q["xp"].as_u64().unwrap())
        .sum();
    assert!(quest_xp >= 50 && xp >= quest_xp + 2 * 10);
}

#[tokio::test]
async fn planning_reminders_books_no_quest_rewards() {
    // 1 December 2026, 12:00 UTC: the day the quests reminder is due.
    let first = START + 15 * DAY;
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (app, _) = app_with(db.clone(), None, first).await;
    let token = session(&app).await;
    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}"), None, None).await;
    // Two check-ins finish a quest. Without an area, checking in doesn't look at quests.
    for place in &places.as_array().unwrap()[..2] {
        let (status, _) = call(&app, "POST", &visits_uri(place), Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
    }
    let booked = || async {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM quest_rewards")
            .fetch_one(&db)
            .await
            .unwrap()
    };
    assert_eq!(booked().await, 0);

    let uri = format!("/api/reminders?{HERE}&days=366");
    let (status, plan) = call(&app, "GET", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let plan = plan.as_array().unwrap();
    let quests = &plan[0];
    assert_eq!(quests["key"], "quests:2026-12");
    assert_eq!(quests["local_date"], "2026-12-01");
    assert_eq!(quests["hour"], 10);
    assert_eq!(quests["title"], "New quests this month");
    assert_eq!(quests["url"], "/?tab=badges");
    assert_eq!(quests["tag"], "quests");
    // A year ahead: every month's quests, soonest first.
    let months = plan.iter().filter(|r| r["tag"] == "quests").count();
    assert_eq!(months, 13);
    let dates: Vec<&str> = plan
        .iter()
        .map(|r| r["local_date"].as_str().unwrap())
        .collect();
    assert!(dates.is_sorted());
    assert_eq!(booked().await, 0);

    // The plan is the same for nobody in particular, and `days` is how far it looks.
    let (status, anonymous) = call(&app, "GET", &uri, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(anonymous.as_array().unwrap().len(), plan.len());
    let uri = format!("/api/reminders?{HERE}&days=0");
    let (_, today) = call(&app, "GET", &uri, Some(&token), None).await;
    assert!(today
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["local_date"] == "2026-12-01"));
    assert_eq!(booked().await, 0);

    // Looking at the quests themselves is what books the finished one.
    let uri = format!("/api/discover?{HERE}");
    call(&app, "GET", &uri, Some(&token), None).await;
    assert!(booked().await > 0);
}

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
    let (app, _) = app_with(db, Some(url), START).await;
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
