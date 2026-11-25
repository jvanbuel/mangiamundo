//! A local copy of the places in and around Belgium, refreshed daily.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sqlx::SqlitePool;

use super::elements::{places, unknown};
use super::queries::import_query;
use super::Overpass;
use crate::geo::Bbox;
use crate::providers::{Boundary, FoundPlace, Lookup, PlacesProvider};
use crate::store;

/// The area kept as a local copy: Belgium plus a margin, so border towns such as
/// Lille, Maastricht and Aachen are covered too.
pub const IMPORT_BBOX: Bbox = Bbox {
    south: 49.4,
    west: 2.4,
    north: 51.6,
    east: 6.5,
};
/// How often the local copy is refreshed, and how soon a failed import is retried.
const IMPORT_EVERY_SECS: i64 = 86_400;
const IMPORT_RETRY: Duration = Duration::from_hours(1);
const IMPORTED_AT: &str = "osm_import_at";

/// Downloads every place in [`IMPORT_BBOX`] into the local copy. One large query a
/// day is kinder to the public servers than one per area, and a failure only delays
/// the refresh instead of breaking a user's request.
pub async fn import(db: &SqlitePool, overpass: &Overpass) -> anyhow::Result<usize> {
    let (places, unknown) = overpass
        .fetch_any_with(
            &import_query(IMPORT_BBOX, 300),
            Duration::from_secs(330),
            None,
            &|| (),
            |elements| {
                let unknown = unknown(&elements);
                (places(elements), unknown)
            },
        )
        .await?;
    let matched = store::replace_imported(db, IMPORT_BBOX, &places).await?;
    tracing::info!(matched, "pending places matched to imported ones");
    store::replace_unknown(db, &unknown).await?;
    tracing::info!(unknown = unknown.len(), "places with unknown food");
    store::set_setting(db, IMPORTED_AT, &store::now().to_string()).await?;
    Ok(places.len())
}

/// Keeps the local copy fresh. Runs for the life of the server.
pub async fn run_imports(db: SqlitePool, overpass: Overpass) {
    loop {
        let last = store::setting(&db, IMPORTED_AT)
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse::<i64>().ok());
        if last.is_none_or(|at| store::now() - at >= IMPORT_EVERY_SECS) {
            match import(&db, &overpass).await {
                Ok(count) => tracing::info!(count, "osm import finished"),
                Err(error) => tracing::warn!(
                    error = format!("{error:#}"),
                    "osm import failed, will retry"
                ),
            }
        }
        tokio::time::sleep(IMPORT_RETRY).await;
    }
}

/// True when the whole box lies inside the import box.
pub(super) fn covered(b: Bbox) -> bool {
    let i = IMPORT_BBOX;
    b.south >= i.south && b.north <= i.north && b.west >= i.west && b.east <= i.east
}

/// The places in an area from the local copy, or `None` when the copy doesn't cover it.
async fn from_import(db: &SqlitePool, area: &Boundary) -> anyhow::Result<Option<Vec<FoundPlace>>> {
    let bbox = area.shape.bbox();
    if !covered(bbox) || store::setting(db, IMPORTED_AT).await?.is_none() {
        return Ok(None);
    }
    let mut places = store::imported_in(db, bbox).await?;
    places.retain(|p| area.shape.contains(p.lat, p.lng));
    Ok(Some(places))
}

/// Answers from the local copy where it covers the area, and asks Overpass elsewhere.
pub struct WithImport {
    db: SqlitePool,
    live: Arc<dyn PlacesProvider>,
}

impl WithImport {
    pub fn new(db: SqlitePool, live: Arc<dyn PlacesProvider>) -> Self {
        Self { db, live }
    }
}

#[async_trait]
impl PlacesProvider for WithImport {
    /// Same name as the live provider: the ids match, so cached areas and visits carry over.
    fn name(&self) -> &'static str {
        self.live.name()
    }

    async fn search(
        &self,
        area: &Boundary,
        on_next_server: &(dyn Fn() + Send + Sync),
    ) -> anyhow::Result<Vec<FoundPlace>> {
        match self.local(area).await? {
            Some(places) => Ok(places),
            None => self.live.search(area, on_next_server).await,
        }
    }

    async fn local(&self, area: &Boundary) -> anyhow::Result<Option<Vec<FoundPlace>>> {
        from_import(&self.db, area).await
    }

    fn expected_secs(&self, area: &Boundary) -> f64 {
        // ponytail: assumes the local copy exists; before the first import a covered area goes live.
        if covered(area.shape.bbox()) {
            1.0
        } else {
            self.live.expected_secs(area)
        }
    }

    /// Always live: the point is to see an edit before the next import.
    async fn place(&self, id: &str) -> anyhow::Result<Lookup> {
        self.live.place(id).await
    }

    async fn nearby(&self, lat: f64, lng: f64, radius_m: f64) -> anyhow::Result<Vec<FoundPlace>> {
        self.live.nearby(lat, lng, radius_m).await
    }
}
