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
    let now = Arc::new(AtomicI64::new(START));
    let clock = now.clone();
    let router = api::router(AppState {
        db: store::connect("sqlite::memory:").await.unwrap(),
        places: Arc::new(Demo),
        areas: Arc::new(Demo),
        cache_ttl_secs: 3600,
        clock: Arc::new(move || clock.load(Ordering::Relaxed)),
        push_key: None,
    });
    (router, now)
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
    let total = |d: &Value| d["countries"].as_array().unwrap().iter().map(|c| c["places"].as_u64().unwrap()).sum::<u64>();
    let (_, city) = call(&app, "GET", "/api/discover?area=demo-leuven", None, None).await;
    let (status, west) = call(&app, "GET", "/api/discover?area=demo-leuven-3000", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(west["area"]["kind"], "postal_code");
    let (_, east) = call(&app, "GET", "/api/discover?area=demo-leuven-3010", None, None).await;
    assert!(total(&west) > 0 && total(&east) > 0);
    assert_eq!(total(&west) + total(&east), total(&city));
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
    // Open places first, then closed, then unknown hours; alphabetical within each group.
    let key = |p: &Value| {
        let group = match p["open_now"].as_bool() { Some(true) => 0, Some(false) => 1, None => 2 };
        (group, p["name"].as_str().unwrap().to_lowercase())
    };
    assert!(places.windows(2).all(|w| key(&w[0]) <= key(&w[1])));
    assert!(places.iter().all(|p| p["open_now"].is_null() == p["hours_text"].is_null()));

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
    let (status, _) = call(&app, "GET", "/api/discover?area=Not%20An%20Id", None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&app, "GET", "/api/discover?area=demo-nowhere", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&app, "GET", "/api/areas/locate?lat=95&lng=4", None, None).await;
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
    let (_, here) = call(&app, "GET", "/api/areas/locate?lat=50.8798&lng=4.71", None, None).await;
    assert_eq!(here["city"]["id"], "demo-leuven");
    assert_eq!(here["postal_code"]["id"], "demo-leuven-3010");
    let (_, nowhere) = call(&app, "GET", "/api/areas/locate?lat=0&lng=0", None, None).await;
    assert!(nowhere["city"].is_null());
}

#[tokio::test]
async fn distances_are_measured_from_the_user_when_given() {
    let app = app().await;
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    let country = discover["countries"][0]["code"].as_str().unwrap().to_string();
    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}&country={country}"), None, None).await;
    let first = &places[0];
    let uri = format!("/api/places?{HERE}&country={country}&lat={}&lng={}", first["lat"], first["lng"]);
    let (_, from_there) = call(&app, "GET", &uri, None, None).await;
    let same = from_there.as_array().unwrap().iter().find(|p| p["id"] == first["id"]).unwrap();
    assert_eq!(same["distance_m"], 0);
}

#[tokio::test]
async fn celebrations_only_list_countries_found_nearby() {
    let app = app().await;
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    let here: Vec<&str> = discover["countries"].as_array().unwrap().iter().map(|c| c["code"].as_str().unwrap()).collect();

    let (status, list) = call(&app, "GET", &format!("/api/celebrations?{HERE}&days=366"), None, None).await;
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
        assert!(celebration["reminder"]["title"].as_str().unwrap().starts_with(celebration["title"].as_str().unwrap()));
        assert!(celebration["reminder"]["body"].as_str().unwrap().contains("near you"));
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
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), Some(&token), None).await;
    let japan = discover["events"].as_array().unwrap().iter().find(|e| e["id"] == "japan-month-2026-12").unwrap();
    assert_eq!(japan["status"], "upcoming");
    assert_eq!(japan["progress"], 0);

    clock.store(START + 20 * DAY, Ordering::Relaxed); // 6 December
    let (_, during) = call(&app, "GET", &format!("/api/discover?{HERE}"), Some(&token), None).await;
    let events = during["events"].as_array().unwrap();
    let levant = events.iter().find(|e| e["id"] == "levant-month-2026-11");
    assert!(levant.is_none(), "an unearned event disappears once it is over");
    let japan = events.iter().find(|e| e["id"] == "japan-month-2026-12").unwrap();
    assert_eq!(japan["status"], "active");
    assert_eq!(japan["target"], 3);
}

#[tokio::test]
async fn visits_between_counts_distinct_places_inside_the_window() {
    use mangiamundo::geo::{Bbox, Shape};
    use mangiamundo::providers::{AreaInfo, AreaKind, Boundary, FoundPlace};
    use mangiamundo::store::{save_area, Area};

    let db = store::connect("sqlite::memory:").await.unwrap();
    let place = |id: &str, countries: Vec<&'static str>| FoundPlace {
        id: id.into(), name: id.into(), lat: 50.0, lng: 4.0, address: None, url: None,
        cuisine: None, opening_hours: None, countries,
    };
    let info = AreaInfo { id: "test-area".into(), name: "Test".into(), kind: AreaKind::City, detail: None, lat: 50.0, lng: 4.0 };
    let shape = Shape::rect(Bbox { south: 49.9, west: 3.9, north: 50.1, east: 4.1 });
    let area = Area { boundary: Boundary { info, shape }, provider: "test".into(), fetched_at: START };
    save_area(&db, &area, &[place("sushi", vec!["JP"]), place("ramen", vec!["JP"]), place("fusion", vec!["JP", "KR"]), place("pasta", vec!["IT"])]).await.unwrap();
    let token = store::create_user(&db).await.unwrap();
    let user = store::user_for_token(&db, &token).await.unwrap().unwrap();

    let (from, until) = (START, START + 30 * DAY);
    store::add_visit(&db, &user, "sushi", false, from - 1).await.unwrap(); // just before
    store::add_visit(&db, &user, "ramen", false, from).await.unwrap(); // first second
    store::add_visit(&db, &user, "fusion", false, until - 1).await.unwrap(); // last second, two countries
    store::add_visit(&db, &user, "pasta", false, from + DAY).await.unwrap(); // wrong country

    let japan = vec!["JP".to_string()];
    assert_eq!(store::visits_between(&db, &user, &japan, from, until).await.unwrap(), 2);
    let east_asia = vec!["JP".to_string(), "KR".to_string()];
    assert_eq!(store::visits_between(&db, &user, &east_asia, from, until).await.unwrap(), 2, "a place counts once");
    assert_eq!(store::visits_between(&db, &user, &[], from, until).await.unwrap(), 0);
}
