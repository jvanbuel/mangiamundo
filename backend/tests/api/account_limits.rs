//! Limits on link codes and new accounts.

use std::sync::atomic::Ordering;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::Value;
use tower::ServiceExt;

use crate::common::{app, app_with_clock, assert_error, call, session};

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

/// `POST /api/session` as Fly's proxy passes it on from `address`.
async fn session_from(app: &Router, address: &str) -> StatusCode {
    let request = Request::builder()
        .method("POST")
        .uri("/api/session")
        .header("fly-client-ip", address)
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn new_accounts_are_limited_per_address_and_hour() {
    let (app, now) = app_with_clock().await;
    for _ in 0..20 {
        assert_eq!(session_from(&app, "203.0.113.7").await, StatusCode::OK);
    }
    assert_eq!(
        session_from(&app, "203.0.113.7").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    let (status, error) = call(&app, "GET", "/api/health", None, None).await;
    assert_eq!((status, &error["ok"]), (StatusCode::OK, &Value::Bool(true)));
    // Someone else is not held up, and one connection's IPv6 addresses count as one.
    assert_eq!(session_from(&app, "203.0.113.8").await, StatusCode::OK);
    for _ in 0..20 {
        assert_eq!(session_from(&app, "2001:db8:1:2::1").await, StatusCode::OK);
    }
    assert_eq!(
        session_from(&app, "2001:db8:1:2:a:b:c:d").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(session_from(&app, "2001:db8:1:3::1").await, StatusCode::OK);
    // The accounts that were made keep working, and an hour later there is room again.
    now.fetch_add(3600, Ordering::Relaxed);
    assert_eq!(session_from(&app, "203.0.113.7").await, StatusCode::OK);
}

/// Without the proxy's header (tests, development) only the limit for everyone applies.
#[tokio::test]
async fn new_accounts_are_limited_for_everyone_together() {
    let app = app().await;
    for _ in 0..1000 {
        let (status, _) = call(&app, "POST", "/api/session", None, None).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, error) = call(&app, "POST", "/api/session", None, None).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_error(&error, "too_many_requests");
}
