//! Signing in with an emailed code, Apple and Google, against fake mail and key servers.

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
const SIGNING_KEY: &str = include_str!("fixtures/test-signing-key.pem");
const SIGNING_KEY_N: &str = include_str!("fixtures/test-signing-key.n");
const APPLE_AUD: &str = "app.mangiamundo";
const GOOGLE_AUD: &str = "web-client.apps.googleusercontent.com";

/// Fake Resend and key servers: the mails sent, and the base URL.
async fn fakes() -> (String, Arc<Mutex<Vec<Value>>>) {
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
        .route("/keys", get(move || std::future::ready(axum::Json(jwks.clone()))));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, fake).await.unwrap() });
    (base, mails)
}

async fn app() -> (Router, Arc<Mutex<Vec<Value>>>) {
    let (base, mails) = fakes().await;
    let db = store::connect("sqlite::memory:").await.unwrap();
    let router = api::router(AppState {
        loads: Default::default(),
        limits: Default::default(),
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

async fn session(app: &Router) -> String {
    let (_, body) = call(app, "POST", "/api/session", None, None).await;
    body["token"].as_str().unwrap().to_string()
}

/// Checks in somewhere, so the account has something to carry over.
async fn check_in(app: &Router, token: &str) {
    let (_, discover) = call(app, "GET", "/api/discover?area=demo-leuven", Some(token), None).await;
    let country = discover["countries"][0]["code"].as_str().unwrap();
    let (_, places) = call(app, "GET", &format!("/api/places?area=demo-leuven&country={country}"), Some(token), None).await;
    let uri = format!("/api/places/{}/visits", places[0]["id"].as_str().unwrap());
    let (status, _) = call(app, "POST", &uri, Some(token), None).await;
    assert_eq!(status, StatusCode::OK);
}

async fn visited(app: &Router, token: &str) -> u64 {
    let (_, discover) = call(app, "GET", "/api/discover?area=demo-leuven", Some(token), None).await;
    discover["places_visited"].as_u64().unwrap()
}

fn id_token(aud: &str, iss: &str, sub: &str, exp_in: i64) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("test".into());
    let claims = json!({
        "iss": iss, "aud": aud, "sub": sub,
        "exp": store::now() + exp_in, "iat": store::now(),
        "email": "Jan@Example.com", "email_verified": "true",
    });
    encode(&header, &claims, &EncodingKey::from_rsa_pem(SIGNING_KEY.as_bytes()).unwrap()).unwrap()
}

/// The code in the latest mail.
fn last_code(mails: &Mutex<Vec<Value>>) -> String {
    let mails = mails.lock().unwrap();
    let subject = mails.last().unwrap()["subject"].as_str().unwrap();
    subject.split_whitespace().next().unwrap().to_string()
}

#[tokio::test]
async fn an_emailed_code_signs_in_and_brings_progress_to_another_device() {
    let (app, mails) = app().await;
    let phone = session(&app).await;
    check_in(&app, &phone).await;

    let (status, _) = call(&app, "POST", "/api/auth/email/code", None, Some(json!({"email": " Jan@Example.com "}))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(mails.lock().unwrap()[0]["to"], json!(["jan@example.com"]));
    let code = last_code(&mails);
    assert_eq!(code.len(), 6);

    // A wrong code doesn't sign in; the right one does, keeping the phone's progress.
    let wrong = if code == "000000" { "111111" } else { "000000" };
    let (status, _) = call(&app, "POST", "/api/auth/email", Some(&phone), Some(json!({"email": "jan@example.com", "code": wrong}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = call(&app, "POST", "/api/auth/email", Some(&phone), Some(json!({"email": "jan@example.com", "code": code}))).await;
    assert_eq!(status, StatusCode::OK);
    let phone = body["token"].as_str().unwrap().to_string();
    assert_eq!(visited(&app, &phone).await, 1);
    let (_, account) = call(&app, "GET", "/api/account", Some(&phone), None).await;
    assert_eq!(account["identities"], json!([{"provider": "email", "email": "jan@example.com"}]));

    // A code works once.
    let (status, _) = call(&app, "POST", "/api/auth/email", None, Some(json!({"email": "jan@example.com", "code": code}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Another device with its own check-in signs in to the same account; both visits count.
    let laptop = session(&app).await;
    check_in(&app, &laptop).await;
    call(&app, "POST", "/api/auth/email/code", None, Some(json!({"email": "jan@example.com"}))).await;
    let code = last_code(&mails);
    let (_, body) = call(&app, "POST", "/api/auth/email", Some(&laptop), Some(json!({"email": "JAN@example.com", "code": code}))).await;
    let laptop = body["token"].as_str().unwrap().to_string();
    let (_, account) = call(&app, "GET", "/api/account", Some(&laptop), None).await;
    assert_eq!(account["devices"].as_array().unwrap().len(), 2);
    assert!(visited(&app, &laptop).await >= 1);
}

#[tokio::test]
async fn codes_are_limited_per_address_and_by_wrong_guesses() {
    let (app, mails) = app().await;
    for _ in 0..3 {
        let (status, _) = call(&app, "POST", "/api/auth/email/code", None, Some(json!({"email": "a@example.com"}))).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
    let (status, body) = call(&app, "POST", "/api/auth/email/code", None, Some(json!({"email": "a@example.com"}))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "too_many_requests");
    let (status, _) = call(&app, "POST", "/api/auth/email/code", None, Some(json!({"email": "not an address"}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Five wrong guesses use the code up, even the right one fails after that.
    let code = last_code(&mails);
    let wrong = if code == "000000" { "111111" } else { "000000" };
    for _ in 0..5 {
        call(&app, "POST", "/api/auth/email", None, Some(json!({"email": "a@example.com", "code": wrong}))).await;
    }
    let (status, _) = call(&app, "POST", "/api/auth/email", None, Some(json!({"email": "a@example.com", "code": code}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn apple_and_google_tokens_are_checked_and_sign_in() {
    let (app, _) = app().await;
    let device = session(&app).await;
    check_in(&app, &device).await;

    // Wrong audience, wrong issuer, expired: all refused, and never as a 401.
    for token in [
        id_token("someone-else", "https://appleid.apple.com", "apple-1", 600),
        id_token(APPLE_AUD, "https://evil.example", "apple-1", 600),
        id_token(APPLE_AUD, "https://appleid.apple.com", "apple-1", -600),
        "not.a.token".to_string(),
    ] {
        let (status, _) = call(&app, "POST", "/api/auth/apple", Some(&device), Some(json!({"id_token": token}))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    let token = id_token(APPLE_AUD, "https://appleid.apple.com", "apple-1", 600);
    let (status, body) = call(&app, "POST", "/api/auth/apple", Some(&device), Some(json!({"id_token": token}))).await;
    assert_eq!(status, StatusCode::OK);
    let device = body["token"].as_str().unwrap().to_string();
    assert_eq!(visited(&app, &device).await, 1, "the device's progress joins the Apple account");

    // Adding Google to the same account, then signing in with Google on a fresh device.
    let google = id_token(GOOGLE_AUD, "accounts.google.com", "google-1", 600);
    let (_, body) = call(&app, "POST", "/api/auth/google", Some(&device), Some(json!({"id_token": google}))).await;
    let device = body["token"].as_str().unwrap().to_string();
    let (_, account) = call(&app, "GET", "/api/account", Some(&device), None).await;
    let providers: Vec<&str> = account["identities"].as_array().unwrap().iter().map(|i| i["provider"].as_str().unwrap()).collect();
    assert_eq!(providers, ["apple", "google"]);

    let (_, body) = call(&app, "POST", "/api/auth/google", None, Some(json!({"id_token": google, "device": "Pixel"}))).await;
    let fresh = body["token"].as_str().unwrap();
    assert_eq!(visited(&app, fresh).await, 1);

    let (_, providers) = call(&app, "GET", "/api/auth/providers", None, None).await;
    assert_eq!(providers, json!({"apple": null, "google": GOOGLE_AUD, "email": true}));
}
