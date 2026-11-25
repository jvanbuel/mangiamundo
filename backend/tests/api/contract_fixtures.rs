//! Writing contract fixtures, and the ones that need an app of their own.

use axum::http::StatusCode;
use mangiamundo::{api, store};
use serde_json::Value;

use crate::common::{call, session, state_with_notes, START};
use crate::fake_osm::{app_with_osm, osm_place};

/// Writes `value` to `contract/<name>.json`.
pub fn write(name: &str, value: &Value) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../contract");
    std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_string_pretty(value).unwrap() + "\n",
    )
    .unwrap();
}

/// An area that is loading: the fetch is begun by hand, and so slow it stays where it began.
pub async fn write_area_progress() {
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (state, _) = state_with_notes(db, None, None, START);
    let loads = state.loads.clone();
    let _loading = loads.begin("demo-leuven");
    loads.step("demo-leuven", 15.0, 95.0, "Looking for restaurants", 1e9);
    let loading = api::router(state);
    let (status, progress) = call(
        &loading,
        "GET",
        "/api/areas/demo-leuven/progress",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    write("area-progress", &progress);
}

/// Reading again from OpenStreetMap: a place that got hours there, and one mapped since
/// that the user had added.
pub async fn write_osm_fixtures() {
    let (osm, _, fake, _) = app_with_osm().await;
    let token = session(&osm).await;
    let mut edited = osm_place(42, "Lalibela", "ethiopian", 50.8798, 4.7005);
    edited.opening_hours = Some("Mo-Su 12:00-22:00".into());
    let found = mangiamundo::providers::Lookup::Found(edited);
    fake.places
        .lock()
        .unwrap()
        .insert("osm-node-42".into(), found);
    let uri = "/api/places/osm-node-42/refresh?lat=50.88&lng=4.7";
    let (status, refreshed) = call(&osm, "POST", uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    write("refresh", &refreshed);
    let new = r#"{"name":"Inti","country":"PE","lat":50.8801,"lng":4.7009,"note":false}"#;
    let (status, _) = call(&osm, "POST", "/api/places", Some(&token), Some(new)).await;
    assert_eq!(status, StatusCode::OK);
    *fake.nearby.lock().unwrap() = vec![
        osm_place(42, "Lalibela", "ethiopian", 50.8798, 4.7005),
        osm_place(43, "Restaurant Inti", "peruvian", 50.8801, 4.7009),
    ];
    let uri = "/api/places/nearby/refresh?lat=50.88&lng=4.7";
    let (status, nearby) = call(&osm, "POST", uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    write("nearby", &nearby);
}
