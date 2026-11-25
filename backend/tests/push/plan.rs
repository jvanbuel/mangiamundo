//! The plan of reminders, and what is sent on the day.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use axum::http::StatusCode;
use mangiamundo::providers::demo::Demo;
use mangiamundo::push::{run_once, Pusher};
use mangiamundo::{api, store, AppState};
use serde_json::Value;

use crate::common::{browser_keys, call, fake_push_service, HERE, START};

#[tokio::test]
async fn the_plan_holds_what_is_sent_on_the_day() {
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
    let token = call(&app, "POST", "/api/session", None, None).await.1["token"]
        .as_str()
        .unwrap()
        .to_string();

    let uri = format!("/api/reminders?{HERE}&days=366");
    let (status, plan) = call(&app, "GET", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let plan = plan.as_array().unwrap();
    let is_celebration = |r: &&Value| r["key"].as_str().unwrap().starts_with("celebration:");
    let planned = plan.iter().find(is_celebration).expect("a celebration");
    let date = planned["local_date"].as_str().unwrap();
    assert_eq!(planned["hour"], 10);

    // It is on the celebration's first day, with the text `/api/celebrations` gives for it.
    let uri = format!("/api/celebrations?{HERE}&days=366");
    let (_, list) = call(&app, "GET", &uri, None, None).await;
    let key = |c: &Value| format!("celebration:{}:{}", c["id"].as_str().unwrap(), date);
    let celebration = list
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["starts"] == date && planned["key"] == key(c))
        .expect("the celebration that starts that day");
    for field in ["title", "body", "url", "tag"] {
        assert_eq!(planned[field], celebration["reminder"][field], "{field}");
    }

    // On that day the push loop sends exactly what the plan has for it.
    let (p256dh, auth) = browser_keys();
    let body = serde_json::json!({
        "endpoint": format!("{push_url}/push/one"),
        "keys": { "p256dh": p256dh, "auth": auth },
        "area": "demo-leuven", "timezone": "UTC",
    });
    let (status, _) = call(
        &app,
        "PUT",
        "/api/push/subscription",
        Some(&token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let day: chrono::NaiveDate = date.parse().unwrap();
    now.store(
        day.and_hms_opt(10, 30, 0).unwrap().and_utc().timestamp(),
        Ordering::Relaxed,
    );
    let mut expected: Vec<&str> = plan
        .iter()
        .filter(|r| r["local_date"] == date)
        .map(|r| r["key"].as_str().unwrap())
        .collect();
    expected.sort_unstable();
    assert_eq!(run_once(&state, &pusher).await.unwrap(), expected.len());
    assert_eq!(service.received.lock().unwrap().len(), expected.len());
    let sent: Vec<String> = sqlx::query_scalar("SELECT key FROM push_sent ORDER BY key")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(sent, expected);
}
