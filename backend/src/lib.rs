pub mod api;
pub mod calendar;
pub mod catalog;
pub mod config;
pub mod game;
pub mod geo;
pub mod hours;
pub mod providers;
pub mod push;
pub mod store;

use std::sync::Arc;

use chrono::NaiveDate;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub places: Arc<dyn providers::PlacesProvider>,
    pub geocoder: Arc<dyn providers::Geocoder>,
    pub cache_ttl_secs: i64,
    /// The current Unix time. Replaceable so tests can pin a moment.
    pub clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    /// The VAPID public key browsers subscribe with, when Web Push is enabled.
    pub push_key: Option<String>,
}

impl AppState {
    pub fn now(&self) -> i64 {
        (self.clock)()
    }

    /// Today's date in UTC.
    pub fn today(&self) -> NaiveDate {
        chrono::DateTime::from_timestamp(self.now(), 0).unwrap_or_default().date_naive()
    }
}
