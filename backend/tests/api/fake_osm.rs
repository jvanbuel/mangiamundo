//! An app whose OpenStreetMap answers what the test sets.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use axum::Router;
use mangiamundo::geo::{Bbox, Shape};
use mangiamundo::providers::demo::Demo;
use mangiamundo::providers::{AreaInfo, AreaKind, Boundary};
use mangiamundo::{api, store, AppState};

use crate::common::{call, HERE, START};

/// A provider whose OpenStreetMap answers the test sets: areas come from the demo provider.
#[derive(Default)]
pub struct FakeOsm {
    pub places: std::sync::Mutex<std::collections::HashMap<String, mangiamundo::providers::Lookup>>,
    pub nearby: std::sync::Mutex<Vec<mangiamundo::providers::FoundPlace>>,
    pub down: std::sync::atomic::AtomicBool,
}

#[async_trait::async_trait]
impl mangiamundo::providers::PlacesProvider for FakeOsm {
    fn name(&self) -> &'static str {
        "osm"
    }
    async fn search(
        &self,
        area: &mangiamundo::providers::Boundary,
        next: &(dyn Fn() + Send + Sync),
    ) -> anyhow::Result<Vec<mangiamundo::providers::FoundPlace>> {
        mangiamundo::providers::PlacesProvider::search(&Demo, area, next).await
    }
    async fn place(&self, id: &str) -> anyhow::Result<mangiamundo::providers::Lookup> {
        anyhow::ensure!(!self.down.load(Ordering::Relaxed), "overpass is down");
        Ok(self
            .places
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .unwrap_or(mangiamundo::providers::Lookup::Gone))
    }
    async fn nearby(
        &self,
        _lat: f64,
        _lng: f64,
        _radius_m: f64,
    ) -> anyhow::Result<Vec<mangiamundo::providers::FoundPlace>> {
        anyhow::ensure!(!self.down.load(Ordering::Relaxed), "overpass is down");
        Ok(self.nearby.lock().unwrap().clone())
    }
}

/// An app on `osm`, with Leuven cached holding one OSM place, osm-node-42 (Lalibela, Ethiopian).
pub async fn app_with_osm() -> (Router, Arc<AtomicI64>, Arc<FakeOsm>, sqlx::SqlitePool) {
    let db = store::connect("sqlite::memory:").await.unwrap();
    let area = store::Area {
        boundary: Boundary {
            info: AreaInfo {
                id: "demo-leuven".into(),
                name: "Leuven".into(),
                kind: AreaKind::City,
                detail: None,
                lat: 50.88,
                lng: 4.7,
            },
            shape: Shape::rect(Bbox {
                south: 50.8,
                west: 4.6,
                north: 50.95,
                east: 4.8,
            }),
        },
        provider: "osm".into(),
        fetched_at: START,
    };
    store::save_area(
        &db,
        &area,
        &[osm_place(42, "Lalibela", "ethiopian", 50.8798, 4.7005)],
    )
    .await
    .unwrap();
    let fake = Arc::new(FakeOsm::default());
    let now = Arc::new(AtomicI64::new(START));
    let clock = now.clone();
    let router = api::router(AppState {
        loads: mangiamundo::areas::Loads::default(),
        limits: mangiamundo::limits::Limits::default(),
        db: db.clone(),
        places: fake.clone(),
        areas: Arc::new(Demo),
        cache_ttl_secs: 3600,
        clock: Arc::new(move || clock.load(Ordering::Relaxed)),
        push_key: None,
        osm_notes_url: None,
        osm_notes_token: None,
        auth: Arc::default(),
    });
    (router, now, fake, db)
}

pub fn osm_place(
    n: u64,
    name: &str,
    cuisine: &str,
    lat: f64,
    lng: f64,
) -> mangiamundo::providers::FoundPlace {
    mangiamundo::providers::FoundPlace {
        id: format!("osm-node-{n}"),
        name: name.into(),
        lat,
        lng,
        address: None,
        url: None,
        cuisine: Some(cuisine.into()),
        opening_hours: None,
        countries: mangiamundo::catalog::countries_for_cuisine_tag(cuisine),
    }
}

pub async fn countries_here(app: &Router, token: &str) -> Vec<String> {
    let (_, discover) = call(
        app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(token),
        None,
    )
    .await;
    discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["code"].as_str().unwrap().to_string())
        .collect()
}
