pub mod account;
pub mod api;
pub mod areas;
pub mod auth;
pub mod calendar;
pub mod catalog;
pub mod config;
pub mod game;
pub mod geo;
pub mod hours;
pub mod progress;
pub mod providers;
pub mod push;
pub mod share;
pub mod store;

use std::sync::Arc;

use chrono::NaiveDate;
use chrono_tz::Tz;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub places: Arc<dyn providers::PlacesProvider>,
    pub areas: Arc<dyn providers::AreaDirectory>,
    pub cache_ttl_secs: i64,
    /// The current Unix time. Replaceable so tests can pin a moment.
    pub clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    /// The VAPID public key browsers subscribe with, when Web Push is enabled.
    pub push_key: Option<String>,
    /// OSM Notes API endpoint for reporting closed places to OpenStreetMap. Off when unset.
    pub osm_notes_url: Option<String>,
    /// Posts notes as the Mangiamundo OSM account when set, anonymously otherwise.
    pub osm_notes_token: Option<String>,
    /// Signing in with Apple, Google or an emailed code.
    pub auth: Arc<auth::Auth>,
    /// Areas being fetched right now, for the clients' progress wheel.
    pub loads: areas::Loads,
    /// Recent requests per user or client address, for limits on the ones that ask
    /// OpenStreetMap.
    pub limits: api::Limits,
}

impl AppState {
    pub fn now(&self) -> i64 {
        (self.clock)()
    }

    /// Today's date in `zone`.
    pub fn today(&self, zone: Tz) -> NaiveDate {
        calendar::local_date(self.now(), zone)
    }
}
