//! Posting shared links and fetching the public pages they make.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

/// A GET of a public page or image: its status, content type and bytes.
pub async fn fetch(app: &Router, uri: &str) -> (StatusCode, String, Vec<u8>) {
    let request = Request::builder()
        .uri(uri)
        .header("host", "mangiamundo.test")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let kind = response
        .headers()
        .get("content-type")
        .map_or_default(|v| v.to_str().unwrap().to_string());
    let bytes = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, kind, bytes)
}

/// The path of a shared link: `https://mangiamundo.test/p/abc` → `/p/abc`.
pub fn path_of(url: &Value) -> String {
    let url = url.as_str().unwrap();
    assert!(url.starts_with("https://mangiamundo.test/"), "{url}");
    url.trim_start_matches("https://mangiamundo.test")
        .to_string()
}

pub async fn post_shared(app: &Router, uri: &str, token: &str) -> (StatusCode, Value) {
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
