//! Web Push reminders, delivered to a fake push service on localhost.

mod common;

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use axum::http::StatusCode;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use mangiamundo::providers::demo::Demo;
use mangiamundo::push::{run_once, Pusher};
use mangiamundo::{api, store, AppState};
use serde_json::Value;

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

#[tokio::test]
async fn tells_subscribers_once_about_new_food_from_the_import() {
    let db = store::connect("sqlite::memory:").await.unwrap();
    let pusher = Pusher::load(&db, None, "mailto:test@example.com".into())
        .await
        .unwrap();
    let state = AppState {
        loads: mangiamundo::areas::Loads::default(),
        limits: mangiamundo::limits::Limits::default(),
        db: db.clone(),
        // As in production: the daily import's copy answers before the live provider.
        places: Arc::new(mangiamundo::providers::osm::WithImport::new(
            db.clone(),
            Arc::new(Demo),
        )),
        areas: Arc::new(Demo),
        cache_ttl_secs: 365 * 86_400,
        clock: Arc::new(|| START),
        push_key: Some(pusher.public_key.clone()),
        osm_notes_url: None,
        osm_notes_token: None,
        auth: Arc::default(),
    };
    let app = api::router(state.clone());
    let (push_url, service) = fake_push_service().await;
    let received = || service.received.lock().unwrap().len();

    let token = call(&app, "POST", "/api/session", None, None).await.1["token"]
        .as_str()
        .unwrap()
        .to_string();
    let session = mangiamundo::account::session_for_token(&db, &token, START).await;
    let user = session.unwrap().unwrap().user;
    let (p256dh, auth) = browser_keys();
    let body = serde_json::json!({
        "endpoint": format!("{push_url}/push/me"),
        "keys": { "p256dh": p256dh, "auth": auth },
        "area": "demo-leuven", "timezone": "UTC",
    });
    assert_eq!(
        call(
            &app,
            "PUT",
            "/api/push/subscription",
            Some(&token),
            Some(body)
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );

    // The first pass takes what Leuven has as known: subscribing sends nothing.
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);

    // Two countries Leuven doesn't have yet. The user already ate the second one elsewhere.
    let present: Vec<String> = store::area_rows(&db, "demo-leuven", None)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.country)
        .collect();
    let mut absent = mangiamundo::catalog::countries()
        .iter()
        .map(|c| c.code.as_str())
        .filter(|c| {
            !present.iter().any(|p| p == c) && mangiamundo::catalog::cuisine_tag(c).is_some()
        });
    let (new, eaten) = (absent.next().unwrap(), absent.next().unwrap());
    sqlx::query("INSERT INTO places (id, name, lat, lng, cuisine, updated_at) VALUES ('far', 'Far', 0, 0, ?, 0)")
        .bind(mangiamundo::catalog::cuisine_tag(eaten))
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO place_countries (place_id, country) VALUES ('far', ?)")
        .bind(eaten)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO visits (user_id, place_id, visited_at, local_date) VALUES (?, 'far', 0, '2026-01-01')").bind(&user).execute(&db).await.unwrap();

    let leuven = store::find_area(&db, "demo-leuven")
        .await
        .unwrap()
        .unwrap()
        .boundary
        .info;
    let imported = |id: &str, country: &str| mangiamundo::providers::FoundPlace {
        id: id.into(),
        name: id.into(),
        lat: leuven.lat,
        lng: leuven.lng,
        address: None,
        url: None,
        cuisine: mangiamundo::catalog::cuisine_tag(country).map(String::from),
        opening_hours: None,
        countries: vec![],
    };
    let bbox = mangiamundo::providers::osm::IMPORT_BBOX;
    store::set_setting(&db, "osm_import_at", "1").await.unwrap();

    // An import that brings only food they already ate: nothing to say.
    let before = received();
    store::replace_imported(&db, bbox, &[imported("osm-node-2", eaten)])
        .await
        .unwrap();
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);
    assert_eq!(received(), before);

    // The next one brings a new country. A failed send is tried again on the next pass.
    store::replace_imported(
        &db,
        bbox,
        &[imported("osm-node-1", new), imported("osm-node-2", eaten)],
    )
    .await
    .unwrap();
    service.status.store(500, Ordering::Relaxed);
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);
    assert_eq!(received(), before + 1);
    service.status.store(201, Ordering::Relaxed);
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 1);
    // Once is enough, also when the country leaves and comes back.
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);
    store::replace_imported(&db, bbox, &[imported("osm-node-2", eaten)])
        .await
        .unwrap();
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);
    store::replace_imported(
        &db,
        bbox,
        &[imported("osm-node-1", new), imported("osm-node-2", eaten)],
    )
    .await
    .unwrap();
    assert_eq!(run_once(&state, &pusher).await.unwrap(), 0);
    assert_eq!(received(), before + 2);
}

/// The demo provider, with a local copy that can't be read the first time it is asked.
struct BrokenOnce(std::sync::atomic::AtomicBool);

#[async_trait::async_trait]
impl mangiamundo::providers::PlacesProvider for BrokenOnce {
    fn name(&self) -> &'static str {
        Demo.name()
    }

    async fn search(
        &self,
        area: &mangiamundo::providers::Boundary,
        on_next_server: &(dyn Fn() + Send + Sync),
    ) -> anyhow::Result<Vec<mangiamundo::providers::FoundPlace>> {
        Demo.search(area, on_next_server).await
    }

    async fn local(
        &self,
        _area: &mangiamundo::providers::Boundary,
    ) -> anyhow::Result<Option<Vec<mangiamundo::providers::FoundPlace>>> {
        if self.0.swap(false, Ordering::Relaxed) {
            anyhow::bail!("the local copy can't be read");
        }
        Ok(None)
    }
}

#[tokio::test]
async fn a_subscriber_whose_reminders_fail_does_not_stop_the_others() {
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
        places: Arc::new(BrokenOnce(true.into())),
        areas: Arc::new(Demo),
        // Short, so the area is stale on the day of the reminder and the local copy is asked.
        cache_ttl_secs: 3600,
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
    let uri = format!("/api/celebrations?{HERE}&days=366");
    let (_, list) = call(&app, "GET", &uri, None, None).await;
    let day: chrono::NaiveDate = list[0]["starts"].as_str().unwrap().parse().unwrap();
    for id in ["one", "two"] {
        let (p256dh, auth) = browser_keys();
        let body = serde_json::json!({
            "endpoint": format!("{push_url}/push/{id}"),
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
    }
    let reminded = || {
        let received = service.received.lock().unwrap();
        let mut ids: Vec<String> = received.iter().map(|(id, _, _)| id.clone()).collect();
        ids.sort();
        ids.dedup();
        ids
    };

    // The first subscriber of the pass fails on the local copy; the other is still reminded.
    now.store(
        day.and_hms_opt(10, 30, 0).unwrap().and_utc().timestamp(),
        Ordering::Relaxed,
    );
    assert!(run_once(&state, &pusher).await.unwrap() >= 1);
    assert_eq!(reminded().len(), 1);

    // The one that failed gets theirs on the next pass.
    assert!(run_once(&state, &pusher).await.unwrap() >= 1);
    assert_eq!(reminded(), ["one", "two"]);
}

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
