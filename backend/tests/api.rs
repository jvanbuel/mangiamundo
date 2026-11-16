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

const HERE: &str = "lat=50.8798&lng=4.7005";

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
        geocoder: Arc::new(Demo),
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
    use mangiamundo::providers::FoundPlace;
    use mangiamundo::store::{area_key, save_area, Area};

    let db = store::connect("sqlite::memory:").await.unwrap();
    let place = |id: &str, countries: Vec<&'static str>| FoundPlace {
        id: id.into(), name: id.into(), lat: 50.0, lng: 4.0, address: None, url: None,
        cuisine: None, countries,
    };
    let area = Area { key: area_key("test", 50.0, 4.0, 1000), lat: 50.0, lng: 4.0, radius_m: 1000, provider: "test".into(), fetched_at: START };
    save_area(&db, &area, &[place("sushi", vec!["JP"]), place("ramen", vec!["JP"]), place("fusion", vec!["JP", "KR"]), place("pasta", vec!["IT"])]).await.unwrap();
    let token = store::create_user(&db).await.unwrap();
    let user = store::user_for_token(&db, &token).await.unwrap().unwrap();

    let (from, until) = (START, START + 30 * DAY);
    store::add_checkin(&db, &user, "sushi", false, None, from - 1).await.unwrap(); // just before
    store::add_checkin(&db, &user, "ramen", false, None, from).await.unwrap(); // first second
    store::add_checkin(&db, &user, "fusion", false, None, until - 1).await.unwrap(); // last second, two countries
    store::add_checkin(&db, &user, "pasta", false, None, from + DAY).await.unwrap(); // wrong country
    store::add_checkin(&db, &user, "ramen", false, None, from + DAY).await.unwrap(); // a second visit, same place

    let japan = vec!["JP".to_string()];
    assert_eq!(store::visits_between(&db, &user, &japan, from, until).await.unwrap(), 2);
    let east_asia = vec!["JP".to_string(), "KR".to_string()];
    assert_eq!(store::visits_between(&db, &user, &east_asia, from, until).await.unwrap(), 2, "a place counts once");
    assert_eq!(store::visits_between(&db, &user, &[], from, until).await.unwrap(), 0);
}

/// The first place in the area and one of its countries' signature dishes.
async fn place_with_dish(app: &Router, token: &str) -> (String, String, String) {
    let (_, catalog) = call(app, "GET", "/api/countries", None, None).await;
    let (_, discover) = call(app, "GET", &format!("/api/discover?{HERE}"), Some(token), None).await;
    for country in discover["countries"].as_array().unwrap() {
        let code = country["code"].as_str().unwrap();
        let Some(dish) = catalog["dishes"].as_array().unwrap().iter().find(|d| d["country"] == code) else {
            continue;
        };
        let (_, places) = call(app, "GET", &format!("/api/places?{HERE}&country={code}"), Some(token), None).await;
        let place = places[0]["id"].as_str().unwrap().to_string();
        return (place, code.to_string(), dish["id"].as_str().unwrap().to_string());
    }
    panic!("no country with dishes in the demo area");
}

#[tokio::test]
async fn returning_counts_again_once_per_cooldown_and_dishes_earn_xp() {
    let (app, clock) = app_with_clock().await;
    let token = session(&app).await;
    let (place, country, dish) = place_with_dish(&app, &token).await;
    let uri = format!("/api/visits/{place}");

    // First visit: 10 + 50 XP.
    let (status, first) = call(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["profile"]["xp"], 60);

    // Too soon to count again.
    let (status, _) = call(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}&country={country}"), Some(&token), None).await;
    let row = places.as_array().unwrap().iter().find(|p| p["id"] == place).unwrap();
    assert_eq!(row["visits"], 1);
    assert_eq!(row["next_check_in_at"], START + 12 * 3600);
    assert_eq!(row["last_checkin_id"], first["checkin_id"]);

    // Logging the signature dish on that visit: +25 XP, once per dish.
    let patch = format!("/api/checkins/{}", first["checkin_id"]);
    let (status, profile) = call(&app, "PATCH", &patch, Some(&token), Some(&format!(r#"{{"dish":"{dish}"}}"#))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(profile["xp"], 85);
    assert_eq!(profile["dishes"], 1);
    let (status, _) = call(&app, "PATCH", &patch, Some(&token), Some(r#"{"dish":"jp-not-a-dish"}"#)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The next day: a second visit with the same dish adds only the visit XP.
    clock.store(START + DAY, std::sync::atomic::Ordering::Relaxed);
    let (status, second) = call(&app, "POST", &uri, Some(&token), Some(&format!(r#"{{"dish":"{dish}"}}"#))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["profile"]["xp"], 95);
    assert_eq!(second["profile"]["visits"], 2);

    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), Some(&token), None).await;
    assert_eq!(discover["dishes_tried"], serde_json::json!([dish]));
    assert_eq!(badge(&discover, "dish-1")["earned"], true);
    assert_eq!(badge(&discover, "regular")["progress"], 2);

    // Undoing the latest check-in leaves the first one.
    let (status, _) = call(&app, "DELETE", &format!("/api/checkins/{}", second["checkin_id"]), Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}&country={country}"), Some(&token), None).await;
    let row = places.as_array().unwrap().iter().find(|p| p["id"] == place).unwrap();
    assert_eq!(row["visits"], 1);
    assert_eq!(row["next_check_in_at"], Value::Null);
    // Someone else cannot touch it.
    let other = session(&app).await;
    let (status, _) = call(&app, "DELETE", &format!("/api/checkins/{}", first["checkin_id"]), Some(&other), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_recovery_code_moves_the_passport_to_another_device() {
    let app = app().await;
    let phone = session(&app).await;
    let (place, _, _) = place_with_dish(&app, &phone).await;
    call(&app, "POST", &format!("/api/visits/{place}"), Some(&phone), None).await;

    let (status, body) = call(&app, "GET", "/api/session/recovery", Some(&phone), None).await;
    assert_eq!(status, StatusCode::OK);
    let code = body["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 19, "four groups of four: {code}");
    let (_, again) = call(&app, "GET", "/api/session/recovery", Some(&phone), None).await;
    assert_eq!(again["code"], code.as_str(), "the code stays the same until rotated");

    // A new laptop that already checked in somewhere else restores the phone's passport.
    let laptop = session(&app).await;
    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}"), Some(&laptop), None).await;
    let elsewhere = places.as_array().unwrap().iter().find(|p| p["id"] != place.as_str()).unwrap()["id"].clone();
    call(&app, "POST", &format!("/api/visits/{}", elsewhere.as_str().unwrap()), Some(&laptop), None).await;

    let typed = code.to_lowercase().replace('-', " ");
    let (status, restored) =
        call(&app, "POST", "/api/session/restore", Some(&laptop), Some(&format!(r#"{{"code":"{typed}"}}"#))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(restored["token"], phone.as_str());
    assert_eq!(restored["merged"], 1);
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), Some(&phone), None).await;
    assert_eq!(discover["profile"]["visits"], 2);
    // The laptop's own account is gone.
    let (status, _) = call(&app, "GET", "/api/session/recovery", Some(&laptop), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Rotating the code retires the old one.
    let (_, rotated) = call(&app, "POST", "/api/session/recovery", Some(&phone), None).await;
    assert_ne!(rotated["code"], code.as_str());
    let (status, _) = call(&app, "POST", "/api/session/restore", None, Some(&format!(r#"{{"code":"{code}"}}"#))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&app, "POST", "/api/session/restore", None, Some(r#"{"code":"nope"}"#)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn share_card_is_a_png() {
    let app = app().await;
    let token = session(&app).await;
    let (place, _, _) = place_with_dish(&app, &token).await;
    call(&app, "POST", &format!("/api/visits/{place}"), Some(&token), None).await;

    for theme in ["light", "dark"] {
        let request = Request::builder()
            .uri(format!("/api/share/card.png?{HERE}&place=Leuven&theme={theme}"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "image/png");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[1..4], b"PNG");
        // Set SHARE_CARD_DIR to look at the result.
        if let Ok(dir) = std::env::var("SHARE_CARD_DIR") {
            std::fs::write(format!("{dir}/card-{theme}.png"), &bytes).unwrap();
        }
    }
}
