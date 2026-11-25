//! Asking the area directory as little as possible, and no client too often.
//!
//! Nominatim answers one request per second for the whole server, so everyone who looks
//! for a location would queue behind everyone else. Most of them stand in an area that is
//! already cached, or where someone stood a moment ago, or search for what someone else
//! searched for. What is left is counted per client, so one that keeps asking uses up its
//! own lookups rather than everyone's turns.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::SqlitePool;

use super::{AreaDirectory, AreaInfo, AreaKind, Boundary, Located, Refused};
use crate::limits::{self, Limits};
use crate::store;

mod memory;

use memory::Memory;

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

#[expect(
    clippy::cast_possible_truncation,
    reason = "thousandths of a degree fit an i32"
)]
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
    async fn located_by_outlines(&self, lat: f64, lng: f64) -> sqlx::Result<Located> {
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
        let mut located = self.located_by_outlines(lat, lng).await?;
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
mod tests;
