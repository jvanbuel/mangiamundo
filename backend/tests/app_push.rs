//! News of food for the iOS and Android apps, delivered to fake APNs and FCM servers.

use std::sync::atomic::{AtomicI64, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use http_body_util::BodyExt;
use mangiamundo::providers::demo::Demo;
use mangiamundo::push::{apns::Apns, fcm::Fcm, run_once, Pusher};
use mangiamundo::{api, store, AppState};
use p256::pkcs8::{EncodePrivateKey, LineEnding};
use serde_json::Value;
use tower::ServiceExt;

const START: i64 = 1_794_830_400; // 16 November 2026, 12:00 UTC

#[derive(Default)]
struct Received {
    /// (path, headers, body)
    requests: Mutex<Vec<(String, HeaderMap, Value)>>,
    status: AtomicU16,
}

/// One server playing APNs, Google's token endpoint and FCM.
async fn fake_push_services() -> (String, Arc<Received>) {
    let received = Arc::new(Received { status: AtomicU16::new(200), ..Default::default() });
    let app = Router::new()
        .route(
            "/3/device/{token}",
            post(|State(r): State<Arc<Received>>, Path(token): Path<String>, headers: HeaderMap, Json(body): Json<Value>| async move {
                r.requests.lock().unwrap().push((format!("apns {token}"), headers, body));
                let status = r.status.load(Ordering::Relaxed);
                let reason = if status == 410 { "Unregistered" } else { "" };
                (StatusCode::from_u16(status).unwrap(), Json(serde_json::json!({ "reason": reason })))
            }),
        )
        .route(
            "/token",
            post(|| async { Json(serde_json::json!({ "access_token": "google-access", "expires_in": 3599 })) }),
        )
        .route(
            "/v1/projects/{project}/messages:send",
            post(|State(r): State<Arc<Received>>, Path(project): Path<String>, headers: HeaderMap, Json(body): Json<Value>| async move {
                r.requests.lock().unwrap().push((format!("fcm {project}"), headers, body));
                let status = r.status.load(Ordering::Relaxed);
                (StatusCode::from_u16(status).unwrap(), Json(serde_json::json!({ "error": { "status": "UNREGISTERED" } })))
            }),
        )
        .with_state(received.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, received)
}

async fn call(app: &Router, method: &str, uri: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
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

fn pusher_keys(url: &str) -> (Apns, Fcm) {
    let http = reqwest::Client::new();
    let ec = p256::SecretKey::random(&mut rand::rngs::OsRng).to_pkcs8_pem(LineEnding::LF).unwrap();
    let apns = Apns::new(http.clone(), &ec, "KEY123".into(), "TEAM123".into(), "app.mangiamundo".into())
        .unwrap()
        .with_url(url);
    let rsa = rsa::RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048).unwrap();
    let rsa = rsa::pkcs8::EncodePrivateKey::to_pkcs8_pem(&rsa, rsa::pkcs8::LineEnding::LF).unwrap();
    let account = serde_json::json!({
        "project_id": "mangiamundo-test",
        "client_email": "push@mangiamundo-test.iam.gserviceaccount.com",
        "private_key": rsa.as_str(),
        "token_uri": format!("{url}/token"),
    });
    let fcm = Fcm::new(http, &account.to_string()).unwrap().with_url(url);
    (apns, fcm)
}

#[tokio::test]
async fn apps_get_news_of_food_through_apns_and_fcm() {
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (url, received) = fake_push_services().await;
    let mut pusher = Pusher::load(&db, None, "mailto:test@example.com".into()).await.unwrap();
    let night = START + 11 * 3600;
    let now = Arc::new(AtomicI64::new(night));
    let clock = now.clone();
    let mut state = AppState {
        loads: Default::default(),
        limits: Default::default(),
        db: db.clone(),
        places: Arc::new(Demo),
        areas: Arc::new(Demo),
        cache_ttl_secs: 365 * 86_400,
        clock: Arc::new(move || clock.load(Ordering::Relaxed)),
        push_key: Some(pusher.public_key.clone()),
        push_apps: pusher.apps(),
        osm_notes_url: None,
        osm_notes_token: None,
    };

    // Without keys the server can't reach the apps, and says so.
    let app = api::router(state.clone());
    let token = call(&app, "POST", "/api/session", None, None).await.1["token"].as_str().unwrap().to_string();
    let device = |platform: &str, token: &str| {
        serde_json::json!({ "platform": platform, "token": token, "area": "demo-leuven", "timezone": "UTC" })
    };
    let (status, _) = call(&app, "PUT", "/api/push/device", Some(&token), Some(device("ios", "abc123"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (apns, fcm) = pusher_keys(&url);
    pusher.apns = Some(Arc::new(apns));
    pusher.fcm = Some(Arc::new(fcm));
    state.push_apps = pusher.apps();
    let app = api::router(state.clone());

    let (status, _) = call(&app, "PUT", "/api/push/device", Some(&token), Some(device("ios", "not a token!"))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&app, "PUT", "/api/push/device", Some(&token), Some(device("ios", "a1b2c3"))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&app, "PUT", "/api/push/device", Some(&token), Some(device("android", "fcm:tok-en_1"))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // The first pass takes what Leuven has, without announcing it.
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);

    // A Georgian restaurant opens in Leuven.
    let mut georgian = mangiamundo::providers::FoundPlace {
        id: "osm-node-1".into(),
        name: "Tbilisi".into(),
        lat: 50.8798,
        lng: 4.7005,
        address: None,
        url: None,
        cuisine: Some("georgian".into()),
        opening_hours: None,
        countries: vec!["GE"],
    };
    let mut area = store::find_area(&db, "demo-leuven").await.unwrap().unwrap();
    // Keep the demo places, which have no Georgian or Eritrean food, and add it.
    let mut places = mangiamundo::providers::PlacesProvider::search(&Demo, &area.boundary, &|| {}).await.unwrap();
    assert!(places.iter().all(|p| !p.countries.contains(&"GE") && !p.countries.contains(&"ER")));
    places.push(georgian.clone());
    area.fetched_at = night;
    store::save_area(&db, &area, &places).await.unwrap();

    // Next morning both apps hear about it, and only about it.
    now.store(START + 22 * 3600 + 1800, Ordering::Relaxed);
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 2);
    {
        let requests = received.requests.lock().unwrap();
        let (_, headers, body) = requests.iter().find(|(path, _, _)| path == "apns a1b2c3").expect("APNs");
        assert!(headers["authorization"].to_str().unwrap().starts_with("Bearer ey"));
        assert_eq!(headers["apns-topic"], "app.mangiamundo");
        assert_eq!(headers["apns-push-type"], "alert");
        assert_eq!(body["aps"]["alert"]["title"], "Georgian food is now near you");
        assert_eq!(body["aps"]["alert"]["body"], "Try Tbilisi in Leuven.");
        assert_eq!(body["url"], "/?country=GE");

        let (_, headers, body) = requests.iter().find(|(path, _, _)| path == "fcm mangiamundo-test").expect("FCM");
        assert_eq!(headers["authorization"], "Bearer google-access");
        assert_eq!(body["message"]["token"], "fcm:tok-en_1");
        assert_eq!(body["message"]["notification"]["title"], "Georgian food is now near you");
        assert_eq!(body["message"]["data"]["url"], "/?country=GE");
        assert_eq!(body["message"]["android"]["notification"]["channel_id"], "new_food");
    }
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0, "once");

    // An uninstalled app is forgotten.
    received.status.store(410, Ordering::Relaxed);
    georgian.id = "osm-node-2".into();
    georgian.cuisine = Some("eritrean".into());
    georgian.countries = vec!["ER"];
    places.push(georgian);
    store::save_area(&db, &area, &places).await.unwrap();
    run_once(&state, &pusher).await.unwrap();
    let left: Vec<String> = store::push_subscriptions(&db).await.unwrap().into_iter().map(|(_, s)| s.endpoint).collect();
    assert_eq!(left, vec!["fcm:fcm:tok-en_1".to_string()], "APNs said 410; FCM's 410 is not a token error");

    // Turning it off in the app removes it.
    let (status, _) = call(&app, "POST", "/api/push/device/delete", Some(&token), Some(serde_json::json!({ "token": "fcm:tok-en_1" }))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(store::push_subscriptions(&db).await.unwrap().is_empty());
}
