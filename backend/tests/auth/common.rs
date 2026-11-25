//! The app with fake mail and key servers, signed test tokens, and calls to the API.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use http_body_util::BodyExt;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use mangiamundo::auth::{Auth, AuthConfig};
use mangiamundo::providers::demo::Demo;
use mangiamundo::{api, store, AppState};
use serde_json::{json, Value};
use tower::ServiceExt;

/// A key made only for these tests; the fake key servers publish its public half.
pub const SIGNING_KEY: &str = include_str!("../fixtures/test-signing-key.pem");
pub const SIGNING_KEY_N: &str = include_str!("../fixtures/test-signing-key.n");
pub const APPLE_AUD: &str = "app.mangiamundo";
pub const GOOGLE_AUD: &str = "web-client.apps.googleusercontent.com";

/// Fake Resend and key servers: the mails sent, and the base URL.
pub async fn fakes() -> (String, Arc<Mutex<Vec<Value>>>) {
    let mails: Arc<Mutex<Vec<Value>>> = Arc::default();
    let seen = mails.clone();
    let jwks = json!({"keys": [{"kty": "RSA", "kid": "test", "alg": "RS256", "use": "sig", "n": SIGNING_KEY_N.trim(), "e": "AQAB"}]});
    let fake = Router::new()
        .route(
            "/emails",
            post(move |axum::Json(body): axum::Json<Value>| async move {
                seen.lock().unwrap().push(body);
                axum::Json(json!({"id": "1"}))
            }),
        )
        .route(
            "/keys",
            get(move || std::future::ready(axum::Json(jwks.clone()))),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, fake).await.unwrap() });
    (base, mails)
}

pub async fn app() -> (Router, Arc<Mutex<Vec<Value>>>) {
    let (base, mails) = fakes().await;
    let db = store::connect("sqlite::memory:").await.unwrap();
    let router = api::router(AppState {
        loads: mangiamundo::areas::Loads::default(),
        limits: mangiamundo::limits::Limits::default(),
        db,
        places: Arc::new(Demo),
        areas: Arc::new(Demo),
        cache_ttl_secs: 3600,
        clock: Arc::new(store::now),
        push_key: None,
        osm_notes_url: None,
        osm_notes_token: None,
        auth: Arc::new(Auth::new(AuthConfig {
            apple_client_ids: vec![APPLE_AUD.into()],
            google_client_ids: vec![GOOGLE_AUD.into()],
            google_web_client_id: Some(GOOGLE_AUD.into()),
            apple_keys_url: format!("{base}/keys"),
            google_keys_url: format!("{base}/keys"),
            resend_url: format!("{base}/emails"),
            resend_key: Some("re_test".into()),
            ..Default::default()
        })),
    });
    (router, mails)
}

pub async fn call(
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

pub async fn session(app: &Router) -> String {
    let (_, body) = call(app, "POST", "/api/session", None, None).await;
    body["token"].as_str().unwrap().to_string()
}

/// Checks in somewhere, so the account has something to carry over.
pub async fn check_in(app: &Router, token: &str) {
    let (_, discover) = call(
        app,
        "GET",
        "/api/discover?area=demo-leuven",
        Some(token),
        None,
    )
    .await;
    let country = discover["countries"][0]["code"].as_str().unwrap();
    let (_, places) = call(
        app,
        "GET",
        &format!("/api/places?area=demo-leuven&country={country}"),
        Some(token),
        None,
    )
    .await;
    let uri = format!("/api/places/{}/visits", places[0]["id"].as_str().unwrap());
    let (status, _) = call(app, "POST", &uri, Some(token), None).await;
    assert_eq!(status, StatusCode::OK);
}

pub async fn visited(app: &Router, token: &str) -> u64 {
    let (_, discover) = call(
        app,
        "GET",
        "/api/discover?area=demo-leuven",
        Some(token),
        None,
    )
    .await;
    discover["places_visited"].as_u64().unwrap()
}

pub fn id_token(aud: &str, iss: &str, sub: &str, exp_in: i64) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("test".into());
    let claims = json!({
        "iss": iss, "aud": aud, "sub": sub,
        "exp": store::now() + exp_in, "iat": store::now(),
        "email": "Jan@Example.com", "email_verified": "true",
        "name": "Jan", "picture": "https://lh3.googleusercontent.com/a/photo",
    });
    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(SIGNING_KEY.as_bytes()).unwrap(),
    )
    .unwrap()
}

/// The code in the latest mail.
pub fn last_code(mails: &Mutex<Vec<Value>>) -> String {
    let mails = mails.lock().unwrap();
    let subject = mails.last().unwrap()["subject"].as_str().unwrap();
    subject.split_whitespace().next().unwrap().to_string()
}
