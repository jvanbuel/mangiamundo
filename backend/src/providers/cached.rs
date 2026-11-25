//! Asking the area directory as little as possible, and no client too often.
//!
//! Nominatim answers one request per second for the whole server, so everyone who looks
//! for a location would queue behind everyone else. Most of them stand in an area that is
//! already cached, or where someone stood a moment ago, or search for what someone else
//! searched for. What is left is counted per client, so one that keeps asking uses up its
//! own lookups rather than everyone's turns.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use sqlx::SqlitePool;

use super::{AreaDirectory, AreaInfo, AreaKind, Boundary, Located, Refused};
use crate::limits::{self, Limits};
use crate::store;

/// How long an answer is kept. Boundaries rarely change, and a restart forgets them all.
const TTL_SECS: i64 = 24 * 60 * 60;

/// How long a search that found nothing is kept: it may have been a typo of a name that
/// is mapped tomorrow, or an answer Nominatim had no time for.
const NOTHING_TTL_SECS: i64 = 10 * 60;

/// How many located cells are kept: a few megabytes at most.
const MAX_ANSWERS: usize = 10_000;

/// How many searches are kept, each with up to ten areas: a few megabytes at most.
const MAX_SEARCHES: usize = 2_000;

/// A longer search is not kept: nobody else will type it.
const MAX_QUERY_BYTES: usize = 100;

/// What one client may ask the directory, as `(max, window in seconds)`: only what could
/// not be answered from memory or the store counts. Enough for someone looking for their
/// city, a few tries in a minute; a client that loops stops after seconds, and gets
/// through a few dozen times an hour at most.
///
/// A search is one request to Nominatim, or up to five for a street.
const SEARCHES: &[(usize, i64)] = &[(10, 60), (60, 3600)];
/// Locating a point is up to four requests, so fewer of those.
const LOCATES: &[(usize, i64)] = &[(5, 60), (30, 3600)];
/// An outline is one request, and an app that opens on a new area asks for it in several
/// requests at once.
const OUTLINES: &[(usize, i64)] = &[(20, 60), (100, 3600)];

/// A point rounded to 3 decimal places: a cell of about 100 m.
type Cell = (i32, i32);

fn cell(lat: f64, lng: f64) -> Cell {
    ((lat * 1000.0).round() as i32, (lng * 1000.0).round() as i32)
}

/// A search as its key: "  LEUVEN " and "leuven" are the same search.
fn normalised(query: &str) -> String {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Answers kept in memory, at most `max` of them, each until its own time.
struct Memory<K, V> {
    max: usize,
    /// Answers with the Unix time each expires at.
    kept: Mutex<HashMap<K, (i64, V)>>,
}

impl<K: Hash + Eq + Clone, V: Clone> Memory<K, V> {
    fn new(max: usize) -> Self {
        Self {
            max,
            kept: Default::default(),
        }
    }

    fn get(&self, key: &K, now: i64) -> Option<V> {
        let kept = self.kept.lock().unwrap();
        let (until, answer) = kept.get(key)?;
        (now < *until).then(|| answer.clone())
    }

    fn put(&self, key: K, answer: V, now: i64, ttl: i64) {
        let mut kept = self.kept.lock().unwrap();
        if kept.len() >= self.max && !kept.contains_key(&key) {
            kept.retain(|_, (until, _)| now < *until);
        }
        if kept.len() >= self.max && !kept.contains_key(&key) {
            // Still full of fresh answers: the one that expires first makes room.
            let first = kept.iter().min_by_key(|(_, (until, _))| *until);
            if let Some(first) = first.map(|(key, _)| key.clone()) {
                kept.remove(&first);
            }
        }
        kept.insert(key, (now + ttl, answer));
    }
}

/// Answers `locate` from memory or from the outlines of cached areas and `search` from
/// memory, and asks the inner directory only for what those don't know, as often as the
/// client asking is allowed to.
pub struct CachedDirectory {
    db: SqlitePool,
    inner: Arc<dyn AreaDirectory>,
    limits: Limits,
    located: Memory<Cell, Located>,
    searches: Memory<String, Vec<AreaInfo>>,
    /// The current Unix time. Replaceable so tests can pin a moment.
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl CachedDirectory {
    pub fn new(db: SqlitePool, inner: Arc<dyn AreaDirectory>, limits: Limits) -> Self {
        Self {
            db,
            inner,
            limits,
            located: Memory::new(MAX_ANSWERS),
            searches: Memory::new(MAX_SEARCHES),
            clock: Arc::new(store::now),
        }
    }

    /// Counts a request to the inner directory against the client being served, and
    /// refuses it when that client made too many. Background work is never refused.
    fn spend(&self, kind: &'static str, budgets: &[(usize, i64)]) -> Result<(), Refused> {
        let Some(client) = limits::client() else {
            return Ok(());
        };
        if self
            .limits
            .allow_each(&client, kind, budgets, (self.clock)())
        {
            Ok(())
        } else {
            tracing::warn!(client, kind, "too many area lookups");
            Err(Refused::TooMany)
        }
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
        let key = normalised(query);
        if let Some(found) = self.searches.get(&key, (self.clock)()) {
            return Ok(found);
        }
        self.spend("area_search", SEARCHES)?;
        // An error is not remembered: the next search asks again.
        let found = self.inner.search(query).await?;
        if key.len() <= MAX_QUERY_BYTES {
            let ttl = if found.is_empty() {
                NOTHING_TTL_SECS
            } else {
                TTL_SECS
            };
            self.searches.put(key, found.clone(), (self.clock)(), ttl);
        }
        Ok(found)
    }

    async fn locate(&self, lat: f64, lng: f64) -> anyhow::Result<Located> {
        // Memory first: it is cheaper than reading the outlines, and as good an answer.
        let cell = cell(lat, lng);
        if let Some(located) = self.located.get(&cell, (self.clock)()) {
            return Ok(located);
        }
        let mut located = self.from_outlines(lat, lng).await?;
        if located.city.is_none() || located.postal_code.is_none() {
            // Half an answer would hide the other half, so the directory gives all of it.
            // An error is not remembered: the next lookup asks again.
            self.spend("area_locate", LOCATES)?;
            located = self.inner.locate(lat, lng).await?;
        }
        self.located
            .put(cell, located.clone(), (self.clock)(), TTL_SECS);
        Ok(located)
    }

    /// Not answered from the store: `areas::resolve` only asks for an outline it has not
    /// cached yet.
    async fn boundary(&self, id: &str) -> anyhow::Result<Option<Boundary>> {
        self.spend("area_outline", OUTLINES)?;
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

    /// Knows one city everywhere and finds it under any name but "nowhere", counts how
    /// often it is asked, and can be down.
    #[derive(Default)]
    struct Inner {
        asked: AtomicUsize,
        down: AtomicBool,
    }

    #[async_trait]
    impl AreaDirectory for Inner {
        async fn search(&self, query: &str) -> anyhow::Result<Vec<AreaInfo>> {
            self.asked.fetch_add(1, Ordering::SeqCst);
            if self.down.load(Ordering::SeqCst) {
                anyhow::bail!("down");
            }
            Ok(if query == "nowhere" {
                Vec::new()
            } else {
                vec![info("osm-relation-1", AreaKind::City)]
            })
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
            self.asked.fetch_add(1, Ordering::SeqCst);
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
            ..CachedDirectory::new(db.clone(), inner.clone(), Limits::default())
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
    async fn a_full_memory_drops_the_answer_that_expires_first() {
        let memory = Memory::new(3);
        for i in 0..=3 {
            memory.put(i, i, i64::from(i), TTL_SECS);
        }
        assert_eq!(memory.kept.lock().unwrap().len(), 3);
        assert_eq!(memory.get(&0, 3), None);
        assert_eq!(memory.get(&1, 3), Some(1));

        // What has expired goes before anything fresh does.
        memory.put(4, 4, TTL_SECS + 2, TTL_SECS);
        assert_eq!(memory.kept.lock().unwrap().len(), 2);
        assert_eq!(memory.get(&3, TTL_SECS + 2), Some(3));
    }

    #[tokio::test]
    async fn a_search_is_remembered_however_it_is_spelled() {
        let s = setup().await;
        let found = s.directory.search("Leuven").await.unwrap();
        assert_eq!(found[0].id, "osm-relation-1");
        assert_eq!(s.directory.search("  leuven ").await.unwrap(), found);
        assert_eq!(s.directory.search("LEUVEN").await.unwrap(), found);
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 1);

        s.directory.search("Sint Niklaas").await.unwrap();
        s.directory.search("sint  niklaas").await.unwrap();
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 2);

        s.now.fetch_add(TTL_SECS, Ordering::SeqCst);
        s.directory.search("leuven").await.unwrap();
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 3, "a day later");
    }

    #[tokio::test]
    async fn a_search_that_found_nothing_is_remembered_for_minutes() {
        let s = setup().await;
        assert!(s.directory.search("nowhere").await.unwrap().is_empty());
        s.now.fetch_add(NOTHING_TTL_SECS - 1, Ordering::SeqCst);
        assert!(s.directory.search("nowhere").await.unwrap().is_empty());
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 1);

        s.now.fetch_add(1, Ordering::SeqCst);
        s.directory.search("nowhere").await.unwrap();
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_failed_or_very_long_search_is_not_remembered() {
        let s = setup().await;
        s.inner.down.store(true, Ordering::SeqCst);
        assert!(s.directory.search("leuven").await.is_err());
        s.inner.down.store(false, Ordering::SeqCst);
        assert!(!s.directory.search("leuven").await.unwrap().is_empty());
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 2);

        let long = "a".repeat(MAX_QUERY_BYTES + 1);
        s.directory.search(&long).await.unwrap();
        s.directory.search(&long).await.unwrap();
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 4);
    }

    fn refused(result: anyhow::Result<impl std::fmt::Debug>) -> bool {
        result.unwrap_err().downcast_ref::<Refused>() == Some(&Refused::TooMany)
    }

    #[tokio::test]
    async fn a_client_that_keeps_searching_is_refused_and_another_is_not() {
        let s = setup().await;
        let looping = "203.0.113.7".to_string();
        limits::as_client(looping.clone(), async {
            for i in 0..10 {
                s.directory.search(&format!("town {i}")).await.unwrap();
            }
            assert!(refused(s.directory.search("town 10").await));
            assert_eq!(s.inner.asked.load(Ordering::SeqCst), 10, "not asked");
            // What is remembered costs nothing, also when nothing is left.
            assert!(!s.directory.search("Town 3").await.unwrap().is_empty());
        })
        .await;

        limits::as_client("198.51.100.1".into(), async {
            s.directory.search("town 10").await.unwrap();
        })
        .await;
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 11);

        // A minute later there is room again, until the hour is used up.
        limits::as_client(looping, async {
            for minute in 1..=5 {
                s.now.fetch_add(60, Ordering::SeqCst);
                for i in 0..10 {
                    let query = format!("village {minute} {i}");
                    s.directory.search(&query).await.unwrap();
                }
            }
            s.now.fetch_add(60, Ordering::SeqCst);
            assert!(refused(s.directory.search("hamlet").await));
            s.now.fetch_add(3600 - 6 * 60, Ordering::SeqCst);
            s.directory.search("hamlet").await.unwrap();
        })
        .await;
    }

    #[tokio::test]
    async fn locating_and_outlines_are_counted_per_client_too() {
        let s = setup().await;
        limits::as_client("203.0.113.7".into(), async {
            for i in 0..5 {
                s.directory.locate(10.0 + f64::from(i), 4.7).await.unwrap();
            }
            assert!(refused(s.directory.locate(20.0, 4.7).await));
            // A cell someone stood in costs nothing.
            s.directory.locate(12.0, 4.7).await.unwrap();
            assert_eq!(s.inner.asked.load(Ordering::SeqCst), 5);

            // Its own count: a client that located a lot can still open an area.
            for _ in 0..20 {
                s.directory.boundary("osm-relation-5").await.unwrap();
            }
            assert!(refused(s.directory.boundary("osm-relation-5").await));
        })
        .await;
    }

    #[tokio::test]
    async fn background_work_is_not_counted() {
        let s = setup().await;
        for i in 0..30 {
            s.directory.search(&format!("town {i}")).await.unwrap();
        }
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 30);
    }
}
