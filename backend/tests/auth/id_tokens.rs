//! Signing in with Apple and Google ID tokens.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::{app, call, check_in, id_token, session, visited, APPLE_AUD, GOOGLE_AUD};

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
        let (status, _) = call(
            &app,
            "POST",
            "/api/auth/apple",
            Some(&device),
            Some(json!({"id_token": token})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    let token = id_token(APPLE_AUD, "https://appleid.apple.com", "apple-1", 600);
    let (status, body) = call(
        &app,
        "POST",
        "/api/auth/apple",
        Some(&device),
        Some(json!({"id_token": token})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let device = body["token"].as_str().unwrap().to_string();
    assert_eq!(
        visited(&app, &device).await,
        1,
        "the device's progress joins the Apple account"
    );

    // Adding Google to the same account, then signing in with Google on a fresh device.
    let google = id_token(GOOGLE_AUD, "accounts.google.com", "google-1", 600);
    let (_, body) = call(
        &app,
        "POST",
        "/api/auth/google",
        Some(&device),
        Some(json!({"id_token": google})),
    )
    .await;
    let device = body["token"].as_str().unwrap().to_string();
    let (_, account) = call(&app, "GET", "/api/account", Some(&device), None).await;
    let providers: Vec<&str> = account["identities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["provider"].as_str().unwrap())
        .collect();
    assert_eq!(providers, ["apple", "google"]);
    let identity = &account["identities"][1];
    assert_eq!(
        (identity["name"].as_str(), identity["picture"].as_str()),
        (
            Some("Jan"),
            Some("https://lh3.googleusercontent.com/a/photo")
        )
    );

    let (_, body) = call(
        &app,
        "POST",
        "/api/auth/google",
        None,
        Some(json!({"id_token": google, "device": "Pixel"})),
    )
    .await;
    let fresh = body["token"].as_str().unwrap();
    assert_eq!(visited(&app, fresh).await, 1);

    let (_, providers) = call(&app, "GET", "/api/auth/providers", None, None).await;
    assert_eq!(
        providers,
        json!({"apple": null, "google": GOOGLE_AUD, "email": true})
    );
}
