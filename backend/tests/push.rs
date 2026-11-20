//! Web Push reminders, delivered to a fake push service on localhost.

use std::sync::atomic::{AtomicI64, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::routing::post;
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use http_body_util::BodyExt;
use mangiamundo::providers::demo::Demo;
use mangiamundo::push::{run_once, Pusher};
use mangiamundo::{api, store, AppState};
use serde_json::Value;
use tower::ServiceExt;

const HERE: &str = "area=demo-leuven";
const START: i64 = 1_794_830_400; // 16 November 2026, 12:00 UTC

#[derive(Default)]
struct PushService {
    received: Mutex<Vec<(String, HeaderMap, usize)>>,
    status: AtomicU16,
}

/// A push service that records what it gets and answers with `status`.
async fn fake_push_service() -> (String, Arc<PushService>) {
    let service = Arc::new(PushService {
        status: AtomicU16::new(201),
        ..Default::default()
    });
    let app = Router::new()
        .route(
            "/push/{id}",
            post(
                |State(s): State<Arc<PushService>>,
                 axum::extract::Path(id): axum::extract::Path<String>,
                 headers: HeaderMap,
                 body: axum::body::Bytes| async move {
                    s.received.lock().unwrap().push((id, headers, body.len()));
                    StatusCode::from_u16(s.status.load(Ordering::Relaxed)).unwrap()
                },
            ),
        )
        .with_state(service.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, service)
}

/// Keys a browser would generate for its subscription.
fn browser_keys() -> (String, String) {
    let mut private = [0u8; 32];
    getrandom::fill(&mut private).unwrap();
    let public =
        web_push::VapidSignatureBuilder::from_base64_no_sub(&URL_SAFE_NO_PAD.encode(private))
            .unwrap()
            .get_public_key();
    let mut auth = [0u8; 16];
    getrandom::fill(&mut auth).unwrap();
    (URL_SAFE_NO_PAD.encode(public), URL_SAFE_NO_PAD.encode(auth))
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
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

#[tokio::test]
async fn sends_each_reminder_once_in_the_subscribers_morning() {
    let db = store::connect("sqlite::memory:").await.unwrap();
    let pusher = Pusher::load(&db, None, "mailto:test@example.com".into())
        .await
        .unwrap();
    let now = Arc::new(AtomicI64::new(START));
    let clock = now.clone();
    let state = AppState {
        loads: Default::default(),
        limits: Default::default(),
        db: db.clone(),
        places: Arc::new(Demo),
        areas: Arc::new(Demo),
        cache_ttl_secs: 365 * 86_400,
        clock: Arc::new(move || clock.load(Ordering::Relaxed)),
        push_key: Some(pusher.public_key.clone()),
        osm_notes_url: None,
        osm_notes_token: None,
    };
    let app = api::router(state.clone());
    let (push_url, service) = fake_push_service().await;

    let (status, key) = call(&app, "GET", "/api/push/key", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        URL_SAFE_NO_PAD
            .decode(key["public_key"].as_str().unwrap())
            .unwrap()
            .len(),
        65
    );
    // The key survives a restart.
    let again = Pusher::load(&db, None, "mailto:test@example.com".into())
        .await
        .unwrap();
    assert_eq!(again.public_key, pusher.public_key);

    let token = call(&app, "POST", "/api/session", None, None).await.1["token"]
        .as_str()
        .unwrap()
        .to_string();
    // The first celebration with food nearby (this also fills the area cache).
    let (_, list) = call(
        &app,
        "GET",
        &format!("/api/celebrations?{HERE}&days=366"),
        None,
        None,
    )
    .await;
    let first = &list.as_array().unwrap()[0];
    let day: chrono::NaiveDate = first["starts"].as_str().unwrap().parse().unwrap();
    let at = |hour: u32| day.and_hms_opt(hour, 30, 0).unwrap().and_utc().timestamp();

    let subscribe = |id: &str, timezone: &str| {
        let (p256dh, auth) = browser_keys();
        serde_json::json!({
            "endpoint": format!("{push_url}/push/{id}"),
            "keys": { "p256dh": p256dh, "auth": auth },
            "area": "demo-leuven", "timezone": timezone,
        })
    };
    let (status, _) = call(
        &app,
        "PUT",
        "/api/push/subscription",
        Some(&token),
        Some(subscribe("utc", "UTC")),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // Auckland is 13 hours ahead: 10:30 UTC is 23:30 there, too late for a reminder.
    call(
        &app,
        "PUT",
        "/api/push/subscription",
        Some(&token),
        Some(subscribe("auckland", "Pacific/Auckland")),
    )
    .await;
    let (status, _) = call(
        &app,
        "PUT",
        "/api/push/subscription",
        Some(&token),
        Some(subscribe("bad", "Mars/Olympus")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &app,
        "PUT",
        "/api/push/subscription",
        None,
        Some(subscribe("anon", "UTC")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Too early in the day: nothing yet.
    now.store(at(8), Ordering::Relaxed);
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);

    // 10:30 UTC: the UTC subscriber gets it, encrypted and signed.
    now.store(at(10), Ordering::Relaxed);
    let sent = run_once(&state, &pusher).await.unwrap();
    assert!(sent >= 1);
    {
        let received = service.received.lock().unwrap();
        assert!(
            received.iter().all(|(id, _, _)| id == "utc"),
            "Auckland is asleep"
        );
        let (_, headers, len) = &received[0];
        assert_eq!(headers["content-encoding"], "aes128gcm");
        assert!(headers["authorization"]
            .to_str()
            .unwrap()
            .starts_with("vapid t="));
        assert!(headers.contains_key("ttl"));
        assert!(*len > 100, "an encrypted payload");
    }

    // Later passes the same day send nothing new.
    now.store(at(14), Ordering::Relaxed);
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);
    assert_eq!(service.received.lock().unwrap().len(), sent);

    // A browser that unsubscribed (410) is forgotten.
    service.status.store(410, Ordering::Relaxed);
    call(
        &app,
        "PUT",
        "/api/push/subscription",
        Some(&token),
        Some(subscribe("gone", "UTC")),
    )
    .await;
    run_once(&state, &pusher).await.unwrap();
    let endpoints: Vec<String> = store::push_subscriptions(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, s)| s.endpoint)
        .collect();
    assert!(!endpoints.iter().any(|e| e.ends_with("/gone")));
    assert!(endpoints.iter().any(|e| e.ends_with("/utc")));

    // Unsubscribing removes it.
    let (status, _) = call(
        &app,
        "POST",
        "/api/push/subscription/delete",
        Some(&token),
        Some(serde_json::json!({ "endpoint": format!("{push_url}/push/utc") })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(store::push_subscriptions(&db)
        .await
        .unwrap()
        .iter()
        .all(|(_, s)| !s.endpoint.ends_with("/utc")));
}

#[tokio::test]
async fn tells_subscribers_once_about_new_food_from_the_import() {
    let db = store::connect("sqlite::memory:").await.unwrap();
    let pusher = Pusher::load(&db, None, "mailto:test@example.com".into())
        .await
        .unwrap();
    let state = AppState {
        loads: Default::default(),
        limits: Default::default(),
        db: db.clone(),
        places: Arc::new(Demo),
        areas: Arc::new(Demo),
        cache_ttl_secs: 365 * 86_400,
        clock: Arc::new(|| START),
        push_key: Some(pusher.public_key.clone()),
        osm_notes_url: None,
        osm_notes_token: None,
    };
    let app = api::router(state.clone());
    let (push_url, service) = fake_push_service().await;
    let received = || service.received.lock().unwrap().len();

    let token = call(&app, "POST", "/api/session", None, None).await.1["token"]
        .as_str()
        .unwrap()
        .to_string();
    let user = store::user_for_token(&db, &token).await.unwrap().unwrap();
    let (p256dh, auth) = browser_keys();
    let body = serde_json::json!({
        "endpoint": format!("{push_url}/push/me"),
        "keys": { "p256dh": p256dh, "auth": auth },
        "area": "demo-leuven", "timezone": "UTC",
    });
    assert_eq!(
        call(
            &app,
            "PUT",
            "/api/push/subscription",
            Some(&token),
            Some(body)
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );

    // The first pass takes what Leuven has as known: subscribing sends nothing.
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);

    // Two countries Leuven doesn't have yet. The user already ate the second one elsewhere.
    let present: Vec<String> = store::area_rows(&db, "demo-leuven", None)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.country)
        .collect();
    let mut absent = mangiamundo::catalog::countries()
        .iter()
        .map(|c| c.code.as_str())
        .filter(|c| {
            !present.iter().any(|p| p == c) && mangiamundo::catalog::cuisine_tag(c).is_some()
        });
    let (new, eaten) = (absent.next().unwrap(), absent.next().unwrap());
    sqlx::query("INSERT INTO places (id, name, lat, lng, cuisine, updated_at) VALUES ('far', 'Far', 0, 0, ?, 0)")
        .bind(mangiamundo::catalog::cuisine_tag(eaten))
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO place_countries (place_id, country) VALUES ('far', ?)")
        .bind(eaten)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO visits (user_id, place_id, visited_at, local_date) VALUES (?, 'far', 0, '2026-01-01')").bind(&user).execute(&db).await.unwrap();

    let leuven = store::find_area(&db, "demo-leuven")
        .await
        .unwrap()
        .unwrap()
        .boundary
        .info;
    let imported = |id: &str, country: &str| mangiamundo::providers::FoundPlace {
        id: id.into(),
        name: id.into(),
        lat: leuven.lat,
        lng: leuven.lng,
        address: None,
        url: None,
        cuisine: mangiamundo::catalog::cuisine_tag(country).map(String::from),
        opening_hours: None,
        countries: vec![],
    };
    let bbox = mangiamundo::providers::osm::IMPORT_BBOX;
    store::set_setting(&db, "osm_import_at", "1").await.unwrap();

    // An import that brings only food they already ate: nothing to say.
    let before = received();
    store::replace_imported(&db, bbox, &[imported("osm-node-2", eaten)])
        .await
        .unwrap();
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);
    assert_eq!(received(), before);

    // The next one brings a new country. A failed send is tried again on the next pass.
    store::replace_imported(
        &db,
        bbox,
        &[imported("osm-node-1", new), imported("osm-node-2", eaten)],
    )
    .await
    .unwrap();
    service.status.store(500, Ordering::Relaxed);
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);
    assert_eq!(received(), before + 1);
    service.status.store(201, Ordering::Relaxed);
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 1);
    // Once is enough, also when the country leaves and comes back.
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);
    store::replace_imported(&db, bbox, &[imported("osm-node-2", eaten)])
        .await
        .unwrap();
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);
    store::replace_imported(
        &db,
        bbox,
        &[imported("osm-node-1", new), imported("osm-node-2", eaten)],
    )
    .await
    .unwrap();
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);
    assert_eq!(received(), before + 2);
}
