//! Area lookups are cached and limited per client, as with OpenStreetMap.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use mangiamundo::providers::demo::Demo;
use mangiamundo::{api, limits, store, AppState};
use serde_json::Value;
use tower::ServiceExt;

use crate::common::START;

/// The demo directory, counting how often it is asked.
#[derive(Default)]
struct CountedAreas(std::sync::atomic::AtomicUsize);

#[async_trait::async_trait]
impl mangiamundo::providers::AreaDirectory for CountedAreas {
    async fn search(&self, query: &str) -> anyhow::Result<Vec<mangiamundo::providers::AreaInfo>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Demo.search(query).await
    }

    async fn locate(&self, lat: f64, lng: f64) -> anyhow::Result<mangiamundo::providers::Located> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Demo.locate(lat, lng).await
    }

    async fn boundary(&self, id: &str) -> anyhow::Result<Option<mangiamundo::providers::Boundary>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Demo.boundary(id).await
    }
}

/// An app whose area directory is cached and limited as the OpenStreetMap one is.
async fn app_with_cached_areas() -> (Router, Arc<CountedAreas>) {
    let db = store::connect("sqlite::memory:").await.unwrap();
    let limits = limits::Limits::default();
    let counted = Arc::new(CountedAreas::default());
    let areas = mangiamundo::providers::cached::CachedDirectory::new(
        db.clone(),
        counted.clone(),
        limits.clone(),
    );
    let router = api::router(AppState {
        loads: mangiamundo::areas::Loads::default(),
        limits,
        db,
        places: Arc::new(Demo),
        areas: Arc::new(areas),
        cache_ttl_secs: 3600,
        clock: Arc::new(|| START),
        push_key: None,
        osm_notes_url: None,
        osm_notes_token: None,
        auth: Arc::default(),
    });
    (router, counted)
}

/// A GET as Fly's proxy passes it on from the client at `ip`.
async fn get_from(app: &Router, ip: &str, uri: &str) -> (StatusCode, Value) {
    let request = Request::get(uri)
        .header("fly-client-ip", ip)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn a_repeated_area_search_asks_the_directory_once() {
    let (app, counted) = app_with_cached_areas().await;
    for query in ["leuv", "Leuv", "%20leuv%20", "leuv"] {
        let uri = format!("/api/areas?q={query}");
        let (status, found) = get_from(&app, "203.0.113.7", &uri).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(found[0]["id"], "demo-leuven");
    }
    let (_, found) = get_from(&app, "198.51.100.1", "/api/areas?q=leuv").await;
    assert_eq!(found[0]["id"], "demo-leuven");
    assert_eq!(counted.0.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_client_looping_on_area_lookups_is_limited_and_others_are_served() {
    let (app, counted) = app_with_cached_areas().await;
    let looping = "203.0.113.7";

    // Searching: ten new searches in a minute, and as many repeated ones as it likes.
    for i in 0..10 {
        let (status, _) = get_from(&app, looping, &format!("/api/areas?q=town{i}")).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = get_from(&app, looping, &format!("/api/areas?q=town{i}")).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, body) = get_from(&app, looping, "/api/areas?q=leuv").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "too_many_requests");
    assert_eq!(counted.0.load(Ordering::SeqCst), 10, "not asked");
    let (status, _) = get_from(&app, looping, "/api/areas?q=town3").await;
    assert_eq!(status, StatusCode::OK, "a remembered search is free");

    let (status, found) = get_from(&app, "198.51.100.1", "/api/areas?q=leuv").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(found[0]["id"], "demo-leuven");
    // Someone on the same IPv6 network is the same client; the next network is not.
    let (status, _) = get_from(&app, "2001:db8:0:1::1", "/api/areas?q=gent").await;
    assert_eq!(status, StatusCode::OK);
    for i in 0..9 {
        get_from(&app, "2001:db8:0:1::2", &format!("/api/areas?q=city{i}")).await;
    }
    let (status, _) = get_from(&app, "2001:db8:0:1::3", "/api/areas?q=brugge").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let (status, _) = get_from(&app, "2001:db8:0:2::3", "/api/areas?q=brugge").await;
    assert_eq!(status, StatusCode::OK);

    // Locating: five new cells in a minute.
    for i in 0..5 {
        let uri = format!("/api/areas/locate?lat=1{i}&lng=4.7");
        assert_eq!(get_from(&app, looping, &uri).await.0, StatusCode::OK);
    }
    let (status, body) = get_from(&app, looping, "/api/areas/locate?lat=50.8798&lng=4.71").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "too_many_requests");
    let (status, _) = get_from(&app, looping, "/api/areas/locate?lat=12&lng=4.7").await;
    assert_eq!(status, StatusCode::OK, "a remembered cell is free");
    let (status, here) = get_from(
        &app,
        "198.51.100.1",
        "/api/areas/locate?lat=50.8798&lng=4.71",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(here["city"]["id"], "demo-leuven");

    // Outlines of areas nobody opened before: twenty in a minute.
    for i in 0..20 {
        let uri = format!("/api/discover?area=demo-nowhere-{i}");
        assert_eq!(get_from(&app, looping, &uri).await.0, StatusCode::NOT_FOUND);
    }
    let (status, body) = get_from(&app, looping, "/api/discover?area=demo-leuven").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "too_many_requests");
    let (status, _) = get_from(&app, "198.51.100.1", "/api/discover?area=demo-leuven").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = get_from(&app, looping, "/api/discover?area=demo-leuven").await;
    assert_eq!(status, StatusCode::OK, "an area that is cached is free");
}
