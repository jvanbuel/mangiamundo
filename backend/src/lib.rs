pub mod api;
pub mod catalog;
pub mod config;
pub mod game;
pub mod geo;
pub mod providers;
pub mod store;

use std::sync::Arc;

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub places: Arc<dyn providers::PlacesProvider>,
    pub geocoder: Arc<dyn providers::Geocoder>,
    pub cache_ttl_secs: i64,
}
