//! Locating a point without asking the area directory each time.
//!
//! Nominatim answers one request per second for the whole server, so everyone who opens
//! the app with GPS would queue behind everyone else. Most of them stand in an area that
//! is already cached, or where someone stood a moment ago.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use sqlx::SqlitePool;

use super::{AreaDirectory, AreaInfo, AreaKind, Boundary, Located};
use crate::store;

/// How long an answer is kept. Boundaries rarely change, and a restart forgets them all.
const TTL_SECS: i64 = 24 * 60 * 60;

/// How many answers are kept: a few megabytes at most.
const MAX_ANSWERS: usize = 10_000;

/// A point rounded to 3 decimal places: a cell of about 100 m.
type Cell = (i32, i32);

fn cell(lat: f64, lng: f64) -> Cell {
    ((lat * 1000.0).round() as i32, (lng * 1000.0).round() as i32)
}

/// Answers `locate` from memory or from the outlines of cached areas, and asks the inner
/// directory only when those don't know both the city and the postal code.
pub struct CachedDirectory {
    db: SqlitePool,
    inner: Arc<dyn AreaDirectory>,
    /// Answers by cell, with the Unix time each was given.
    answers: Mutex<HashMap<Cell, (i64, Located)>>,
    /// The current Unix time. Replaceable so tests can pin a moment.
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl CachedDirectory {
    pub fn new(db: SqlitePool, inner: Arc<dyn AreaDirectory>) -> Self {
        Self {
            db,
            inner,
            answers: Default::default(),
            clock: Arc::new(store::now),
        }
    }

    fn remembered(&self, cell: Cell) -> Option<Located> {
        let answers = self.answers.lock().unwrap();
        let (at, located) = answers.get(&cell)?;
        ((self.clock)() - at < TTL_SECS).then(|| located.clone())
    }

    fn remember(&self, cell: Cell, located: &Located) {
        let now = (self.clock)();
        let mut answers = self.answers.lock().unwrap();
        if answers.len() >= MAX_ANSWERS && !answers.contains_key(&cell) {
            answers.retain(|_, (at, _)| now - *at < TTL_SECS);
        }
        if answers.len() >= MAX_ANSWERS && !answers.contains_key(&cell) {
            // Still full of fresh answers: the oldest one makes room.
            let oldest = answers.iter().min_by_key(|(_, (at, _))| *at);
            if let Some(oldest) = oldest.map(|(cell, _)| *cell) {
                answers.remove(&oldest);
            }
        }
        answers.insert(cell, (now, located.clone()));
    }

    /// The city and postal code among the cached areas around the point.
    async fn from_outlines(&self, lat: f64, lng: f64) -> sqlx::Result<Located> {
        let around = store::areas_containing(&self.db, lat, lng).await?;
        Ok(Located {
            city: smallest(&around, AreaKind::City),
            postal_code: smallest(&around, AreaKind::PostalCode),
        })
    }
}

/// When several areas of a kind hold the point, the smallest one, as Nominatim prefers a
/// city, town or village over the municipality around it; the lowest id on a tie.
fn smallest(areas: &[Boundary], kind: AreaKind) -> Option<AreaInfo> {
    let size = |b: &Boundary| {
        let bbox = b.shape.bbox();
        (bbox.north - bbox.south) * (bbox.east - bbox.west)
    };
    areas
        .iter()
        .filter(|b| b.info.kind == kind)
        .min_by(|a, b| {
            size(a)
                .total_cmp(&size(b))
                .then_with(|| a.info.id.cmp(&b.info.id))
        })
        .map(|b| b.info.clone())
}

#[async_trait]
impl AreaDirectory for CachedDirectory {
    async fn search(&self, query: &str) -> anyhow::Result<Vec<AreaInfo>> {
        self.inner.search(query).await
    }

    async fn locate(&self, lat: f64, lng: f64) -> anyhow::Result<Located> {
        // Memory first: it is cheaper than reading the outlines, and as good an answer.
        let cell = cell(lat, lng);
        if let Some(located) = self.remembered(cell) {
            return Ok(located);
        }
        let mut located = self.from_outlines(lat, lng).await?;
        if located.city.is_none() || located.postal_code.is_none() {
            // Half an answer would hide the other half, so the directory gives all of it.
            // An error is not remembered: the next lookup asks again.
            located = self.inner.locate(lat, lng).await?;
        }
        self.remember(cell, &located);
        Ok(located)
    }

    /// Not answered from the store: `areas::resolve` only asks for an outline it has not
    /// cached yet.
    async fn boundary(&self, id: &str) -> anyhow::Result<Option<Boundary>> {
        self.inner.boundary(id).await
    }

    async fn suggest(
        &self,
        query: &str,
        near: Option<(f64, f64)>,
    ) -> anyhow::Result<Vec<AreaInfo>> {
        self.inner.suggest(query, near).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};

    use super::*;
    use crate::geo::{Bbox, Shape};

    /// Knows one city everywhere, counts how often it is asked, and can be down.
    #[derive(Default)]
    struct Inner {
        asked: AtomicUsize,
        down: AtomicBool,
    }

    #[async_trait]
    impl AreaDirectory for Inner {
        async fn search(&self, _query: &str) -> anyhow::Result<Vec<AreaInfo>> {
            Ok(Vec::new())
        }

        async fn locate(&self, _lat: f64, _lng: f64) -> anyhow::Result<Located> {
            self.asked.fetch_add(1, Ordering::SeqCst);
            if self.down.load(Ordering::SeqCst) {
                anyhow::bail!("down");
            }
            Ok(Located {
                city: Some(info("osm-relation-1", AreaKind::City)),
                postal_code: None,
            })
        }

        async fn boundary(&self, _id: &str) -> anyhow::Result<Option<Boundary>> {
            Ok(None)
        }
    }

    fn info(id: &str, kind: AreaKind) -> AreaInfo {
        AreaInfo {
            id: id.into(),
            name: id.into(),
            kind,
            detail: None,
            lat: 50.88,
            lng: 4.7,
        }
    }

    /// Caches an area that is a square reaching `half` degrees from the centre of Leuven.
    async fn cache_area(db: &SqlitePool, id: &str, kind: AreaKind, half: f64) {
        let area = store::Area {
            boundary: Boundary {
                info: info(id, kind),
                shape: Shape::rect(Bbox {
                    south: 50.88 - half,
                    west: 4.7 - half,
                    north: 50.88 + half,
                    east: 4.7 + half,
                }),
            },
            provider: "osm".into(),
            fetched_at: 1000,
        };
        store::save_area(db, &area, &[]).await.unwrap();
    }

    struct Setup {
        db: SqlitePool,
        inner: Arc<Inner>,
        now: Arc<AtomicI64>,
        directory: CachedDirectory,
    }

    async fn setup() -> Setup {
        let db = store::connect("sqlite::memory:").await.unwrap();
        let inner = Arc::new(Inner::default());
        let now = Arc::new(AtomicI64::new(1_000_000));
        let clock = now.clone();
        let directory = CachedDirectory {
            clock: Arc::new(move || clock.load(Ordering::SeqCst)),
            ..CachedDirectory::new(db.clone(), inner.clone())
        };
        Setup {
            db,
            inner,
            now,
            directory,
        }
    }

    #[tokio::test]
    async fn a_point_in_a_cached_city_and_postal_code_needs_no_lookup() {
        let s = setup().await;
        cache_area(&s.db, "osm-relation-10", AreaKind::City, 0.1).await;
        cache_area(&s.db, "osm-relation-20", AreaKind::PostalCode, 0.02).await;

        let here = s.directory.locate(50.881, 4.701).await.unwrap();
        assert_eq!(here.city.unwrap().id, "osm-relation-10");
        assert_eq!(here.postal_code.unwrap().id, "osm-relation-20");
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn the_smallest_cached_area_of_a_kind_wins() {
        let s = setup().await;
        cache_area(&s.db, "osm-relation-10", AreaKind::City, 0.3).await;
        cache_area(&s.db, "osm-relation-12", AreaKind::City, 0.1).await;
        cache_area(&s.db, "osm-relation-11", AreaKind::City, 0.1).await;
        cache_area(&s.db, "osm-relation-20", AreaKind::PostalCode, 0.02).await;

        let here = s.directory.locate(50.881, 4.701).await.unwrap();
        assert_eq!(here.city.unwrap().id, "osm-relation-11");
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn half_an_answer_from_the_outlines_is_not_used() {
        let s = setup().await;
        cache_area(&s.db, "osm-relation-10", AreaKind::City, 0.1).await;

        let here = s.directory.locate(50.881, 4.701).await.unwrap();
        assert_eq!(here.city.unwrap().id, "osm-relation-1", "the directory's");
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_lookup_is_remembered_for_its_cell() {
        let s = setup().await;
        s.directory.locate(50.8812, 4.7012).await.unwrap();
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 1);

        let again = s.directory.locate(50.8814, 4.7009).await.unwrap();
        assert_eq!(again.city.unwrap().id, "osm-relation-1");
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 1, "same cell");

        s.directory.locate(50.8830, 4.7012).await.unwrap();
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 2, "another cell");
    }

    #[tokio::test]
    async fn an_error_is_not_remembered() {
        let s = setup().await;
        s.inner.down.store(true, Ordering::SeqCst);
        assert!(s.directory.locate(50.881, 4.701).await.is_err());

        s.inner.down.store(false, Ordering::SeqCst);
        assert!(s.directory.locate(50.881, 4.701).await.is_ok());
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn an_answer_expires_after_a_day() {
        let s = setup().await;
        s.directory.locate(50.881, 4.701).await.unwrap();
        s.now.fetch_add(TTL_SECS - 1, Ordering::SeqCst);
        s.directory.locate(50.881, 4.701).await.unwrap();
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 1);

        s.now.fetch_add(1, Ordering::SeqCst);
        s.directory.locate(50.881, 4.701).await.unwrap();
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_full_memory_drops_its_oldest_answer() {
        let s = setup().await;
        for i in 0..=MAX_ANSWERS as i32 {
            s.now.store(i64::from(i), Ordering::SeqCst);
            s.directory.remember((i, 0), &Located::default());
        }
        assert_eq!(s.directory.answers.lock().unwrap().len(), MAX_ANSWERS);
        assert!(s.directory.remembered((0, 0)).is_none());
        assert!(s.directory.remembered((1, 0)).is_some());
    }
}
