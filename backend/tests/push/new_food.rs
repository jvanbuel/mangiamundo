//! Telling subscribers about new food from the import.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::http::StatusCode;
use mangiamundo::providers::demo::Demo;
use mangiamundo::push::{run_once, Pusher};
use mangiamundo::{api, store, AppState};

use crate::common::{browser_keys, call, fake_push_service, START};

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
