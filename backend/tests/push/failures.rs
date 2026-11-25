//! One subscriber's failing reminders do not stop the others'.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use axum::http::StatusCode;
use mangiamundo::providers::demo::Demo;
use mangiamundo::push::{run_once, Pusher};
use mangiamundo::{api, store, AppState};

use crate::common::{browser_keys, call, fake_push_service, HERE, START};

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
