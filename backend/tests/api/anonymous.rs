//! What works without an account: health, the catalog and discovering an area.

use axum::http::StatusCode;
use mangiamundo::store;

use crate::common::{app, app_with, assert_error, call, session, HERE, START};

#[tokio::test]
async fn health_reports_the_server_version() {
    let app = app().await;
    let (status, body) = call(&app, "GET", "/api/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert!(body.get("commit").is_some());
}

#[tokio::test]
async fn catalog_lists_countries_and_continents() {
    let app = app().await;
    let (status, body) = call(&app, "GET", "/api/countries", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["countries"].as_array().unwrap().len() > 200);
    assert_eq!(body["continents"].as_array().unwrap().len(), 6);
}

#[tokio::test]
async fn discover_works_anonymously_and_is_stable() {
    let app = app().await;
    let (status, first) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(first["world"]["available"].as_u64().unwrap() > 10);
    assert_eq!(first["world"]["discovered"], 0);
    assert_eq!(first["profile"]["xp"], 0);
    assert_eq!(first["area"]["id"], "demo-leuven");
    assert_eq!(first["area"]["name"], "Leuven");
    assert_eq!(first["area"]["kind"], "city");
    // The second request is served from the cache.
    let (_, second) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    assert_eq!(first["countries"], second["countries"]);
    assert_eq!(first["area"]["fetched_at"], second["area"]["fetched_at"]);
}

/// What a client shows before the first check-in needs no account, and makes none.
#[tokio::test]
async fn everything_a_client_reads_at_first_works_without_an_account() {
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (app, _) = app_with(db.clone(), None, START);
    let (status, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    assert_eq!(status, StatusCode::OK);
    let country = discover["countries"][0]["code"].as_str().unwrap();
    let (status, places) = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}&country={country}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(places[0]["visits"], serde_json::json!([]));
    let place = places[0]["id"].as_str().unwrap();
    let reads = [
        format!("/api/places/{place}"),
        format!("/api/suggestion?{HERE}&open=false"),
        format!("/api/unknown-places?{HERE}"),
        format!("/api/reminders?{HERE}"),
        format!("/api/celebrations?{HERE}"),
        "/api/countries".to_string(),
        "/api/auth/providers".to_string(),
    ];
    for uri in &reads {
        let (status, body) = call(&app, "GET", uri, None, None).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
    }
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(users, 0);

    // A token the server doesn't know is not anonymous: the device was signed out, and
    // its app says so.
    let token = session(&app).await;
    let (_, account) = call(&app, "GET", "/api/account", Some(&token), None).await;
    let device = account["devices"][0]["id"].as_str().unwrap();
    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/account/devices/{device}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    for uri in &reads[..4] {
        let (status, body) = call(&app, "GET", uri, Some(&token), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert_error(&body, "unauthorized");
    }
    // A write needs an account.
    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/places/{place}/visits?{HERE}"),
        None,
        Some("{}"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
