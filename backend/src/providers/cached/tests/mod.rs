use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};

use super::*;
use crate::geo::{Bbox, Shape};

mod clients;
mod locate;
mod memory;
mod search;

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

fn refused(result: anyhow::Result<impl std::fmt::Debug>) -> bool {
    result.unwrap_err().downcast_ref::<Refused>() == Some(&Refused::TooMany)
}
