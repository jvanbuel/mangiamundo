//! Signing in with an emailed code, Apple and Google, against fake mail and key servers.

mod common;

use axum::http::StatusCode;
use serde_json::json;

use crate::common::{
    app, call, check_in, id_token, last_code, session, visited, APPLE_AUD, GOOGLE_AUD,
};

#[tokio::test]
async fn an_emailed_code_signs_in_and_brings_progress_to_another_device() {
    let (app, mails) = app().await;
    let phone = session(&app).await;
    check_in(&app, &phone).await;

    let (status, _) = call(
        &app,
        "POST",
        "/api/auth/email/code",
        None,
        Some(json!({"email": " Jan@Example.com "})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(mails.lock().unwrap()[0]["to"], json!(["jan@example.com"]));
    let code = last_code(&mails);
    assert_eq!(code.len(), 6);

    // A wrong code doesn't sign in; the right one does, keeping the phone's progress.
    let wrong = if code == "000000" { "111111" } else { "000000" };
    let (status, _) = call(
        &app,
        "POST",
        "/api/auth/email",
        Some(&phone),
        Some(json!({"email": "jan@example.com", "code": wrong})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = call(
        &app,
        "POST",
        "/api/auth/email",
        Some(&phone),
        Some(json!({"email": "jan@example.com", "code": code})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let phone = body["token"].as_str().unwrap().to_string();
    assert_eq!(visited(&app, &phone).await, 1);
    let (_, account) = call(&app, "GET", "/api/account", Some(&phone), None).await;
    assert_eq!(
        account["identities"],
        json!([{"provider": "email", "email": "jan@example.com", "name": null, "picture": null}])
    );

    // A code works once.
    let (status, _) = call(
        &app,
        "POST",
        "/api/auth/email",
        None,
        Some(json!({"email": "jan@example.com", "code": code})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Another device with its own check-in signs in to the same account; both visits count.
    let laptop = session(&app).await;
    check_in(&app, &laptop).await;
    call(
        &app,
        "POST",
        "/api/auth/email/code",
        None,
        Some(json!({"email": "jan@example.com"})),
    )
    .await;
    let code = last_code(&mails);
    let (_, body) = call(
        &app,
        "POST",
        "/api/auth/email",
        Some(&laptop),
        Some(json!({"email": "JAN@example.com", "code": code})),
    )
    .await;
    let laptop = body["token"].as_str().unwrap().to_string();
    let (_, account) = call(&app, "GET", "/api/account", Some(&laptop), None).await;
    assert_eq!(account["devices"].as_array().unwrap().len(), 2);
    assert!(visited(&app, &laptop).await >= 1);
}

#[tokio::test]
async fn codes_are_limited_per_address_and_by_wrong_guesses() {
    let (app, mails) = app().await;
    for _ in 0..3 {
        let (status, _) = call(
            &app,
            "POST",
            "/api/auth/email/code",
            None,
            Some(json!({"email": "a@example.com"})),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
    let (status, body) = call(
        &app,
        "POST",
        "/api/auth/email/code",
        None,
        Some(json!({"email": "a@example.com"})),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "too_many_requests");
    let (status, _) = call(
        &app,
        "POST",
        "/api/auth/email/code",
        None,
        Some(json!({"email": "not an address"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Five wrong guesses use the code up, even the right one fails after that.
    let code = last_code(&mails);
    let wrong = if code == "000000" { "111111" } else { "000000" };
    for _ in 0..5 {
        call(
            &app,
            "POST",
            "/api/auth/email",
            None,
            Some(json!({"email": "a@example.com", "code": wrong})),
        )
        .await;
    }
    let (status, _) = call(
        &app,
        "POST",
        "/api/auth/email",
        None,
        Some(json!({"email": "a@example.com", "code": code})),
    )
    .await;
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
