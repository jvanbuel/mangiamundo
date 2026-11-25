//! Bad input and unknown paths: every error is JSON with a code.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use mangiamundo::api;
use tower::ServiceExt;

use crate::common::{app, assert_error, call, session, HERE};

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
