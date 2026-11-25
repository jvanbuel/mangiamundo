//! The fake push service, the keys a browser subscribes with, and calls to the API.

use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::routing::post;
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

pub const HERE: &str = "area=demo-leuven";
pub const START: i64 = 1_794_830_400; // 16 November 2026, 12:00 UTC

#[derive(Default)]
pub struct PushService {
    pub received: Mutex<Vec<(String, HeaderMap, usize)>>,
    pub status: AtomicU16,
}

/// A push service that records what it gets and answers with `status`.
pub async fn fake_push_service() -> (String, Arc<PushService>) {
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
pub fn browser_keys() -> (String, String) {
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
