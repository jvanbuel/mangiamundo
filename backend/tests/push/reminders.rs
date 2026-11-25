//! Reminders sent once each, in the subscriber's morning.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use axum::http::StatusCode;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use mangiamundo::providers::demo::Demo;
use mangiamundo::push::{run_once, Pusher};
use mangiamundo::{api, store, AppState};

use crate::common::{browser_keys, call, fake_push_service, HERE, START};

#[tokio::test]
async fn sends_each_reminder_once_in_the_subscribers_morning() {
    let db = store::connect("sqlite::memory:").await.unwrap();
    let pusher = Pusher::load(&db, None, "mailto:test@example.com".into())
        .await
        .unwrap();
    let now = Arc::new(AtomicI64::new(START));
    let clock = now.clone();
    let state = AppState {
        loads: mangiamundo::areas::Loads::default(),
        limits: mangiamundo::limits::Limits::default(),
        db: db.clone(),
        places: Arc::new(Demo),
        areas: Arc::new(Demo),
        cache_ttl_secs: 365 * 86_400,
        clock: Arc::new(move || clock.load(Ordering::Relaxed)),
        push_key: Some(pusher.public_key.clone()),
        osm_notes_url: None,
        osm_notes_token: None,
        auth: Arc::default(),
    };
    let app = api::router(state.clone());
    let (push_url, service) = fake_push_service().await;

    let (status, key) = call(&app, "GET", "/api/push/key", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        URL_SAFE_NO_PAD
            .decode(key["public_key"].as_str().unwrap())
            .unwrap()
            .len(),
        65
    );
    // The key survives a restart.
    let again = Pusher::load(&db, None, "mailto:test@example.com".into())
        .await
        .unwrap();
    assert_eq!(again.public_key, pusher.public_key);

    let token = call(&app, "POST", "/api/session", None, None).await.1["token"]
        .as_str()
        .unwrap()
        .to_string();
    // The first celebration with food nearby (this also fills the area cache).
    let (_, list) = call(
        &app,
        "GET",
        &format!("/api/celebrations?{HERE}&days=366"),
        None,
        None,
    )
    .await;
    let first = &list.as_array().unwrap()[0];
    let day: chrono::NaiveDate = first["starts"].as_str().unwrap().parse().unwrap();
    let at = |hour: u32| day.and_hms_opt(hour, 30, 0).unwrap().and_utc().timestamp();

    let subscribe = |id: &str, timezone: &str| {
        let (p256dh, auth) = browser_keys();
        serde_json::json!({
            "endpoint": format!("{push_url}/push/{id}"),
            "keys": { "p256dh": p256dh, "auth": auth },
            "area": "demo-leuven", "timezone": timezone,
        })
    };
    let (status, _) = call(
        &app,
        "PUT",
        "/api/push/subscription",
        Some(&token),
        Some(subscribe("utc", "UTC")),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // Auckland is 13 hours ahead: 10:30 UTC is 23:30 there, too late for a reminder.
    call(
        &app,
        "PUT",
        "/api/push/subscription",
        Some(&token),
        Some(subscribe("auckland", "Pacific/Auckland")),
    )
    .await;
    let (status, _) = call(
        &app,
        "PUT",
        "/api/push/subscription",
        Some(&token),
        Some(subscribe("bad", "Mars/Olympus")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &app,
        "PUT",
        "/api/push/subscription",
        None,
        Some(subscribe("anon", "UTC")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Too early in the day: nothing yet.
    now.store(at(8), Ordering::Relaxed);
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);

    // 10:30 UTC: the UTC subscriber gets it, encrypted and signed.
    now.store(at(10), Ordering::Relaxed);
    let sent = run_once(&state, &pusher).await.unwrap();
    assert!(sent >= 1);
    {
        let received = service.received.lock().unwrap();
        assert!(
            received.iter().all(|(id, _, _)| id == "utc"),
            "Auckland is asleep"
        );
        let (_, headers, len) = &received[0];
        assert_eq!(headers["content-encoding"], "aes128gcm");
        assert!(headers["authorization"]
            .to_str()
            .unwrap()
            .starts_with("vapid t="));
        assert!(headers.contains_key("ttl"));
        assert!(*len > 100, "an encrypted payload");
    }

    // Later passes the same day send nothing new.
    now.store(at(14), Ordering::Relaxed);
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);
    assert_eq!(service.received.lock().unwrap().len(), sent);

    // A browser that unsubscribed (410) is forgotten.
    service.status.store(410, Ordering::Relaxed);
    call(
        &app,
        "PUT",
        "/api/push/subscription",
        Some(&token),
        Some(subscribe("gone", "UTC")),
    )
    .await;
    run_once(&state, &pusher).await.unwrap();
    let endpoints: Vec<String> = store::push_subscriptions(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, s)| s.endpoint)
        .collect();
    assert!(!endpoints.iter().any(|e| e.ends_with("/gone")));
    assert!(endpoints.iter().any(|e| e.ends_with("/utc")));

    // Unsubscribing removes it.
    let (status, _) = call(
        &app,
        "POST",
        "/api/push/subscription/delete",
        Some(&token),
        Some(serde_json::json!({ "endpoint": format!("{push_url}/push/utc") })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(store::push_subscriptions(&db)
        .await
        .unwrap()
        .iter()
        .all(|(_, s)| !s.endpoint.ends_with("/utc")));
}
