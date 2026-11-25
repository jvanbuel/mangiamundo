//! What the server is: its health and the countries the game knows.

use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

use crate::catalog;

#[derive(Serialize, ToSchema)]
pub(super) struct Health {
    ok: bool,
    /// The server's Cargo version.
    version: &'static str,
    /// The git commit it was built from: `GIT_COMMIT` at compile time (set by the
    /// Dockerfile), `null` in builds without it.
    #[schema(required = true)]
    commit: Option<&'static str>,
}

/// Liveness check, also telling clients which build of the server they talk to.
#[utoipa::path(get, path = "/api/health", tag = "meta", responses((status = 200, body = Health)))]
pub(super) async fn health() -> Json<Health> {
    Json(Health {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
        commit: option_env!("GIT_COMMIT"),
    })
}

#[derive(Serialize, ToSchema)]
pub(super) struct Catalog {
    countries: &'static [catalog::Country],
    continents: Vec<Continent>,
}

#[derive(Serialize, ToSchema)]
struct Continent {
    code: &'static str,
    name: &'static str,
}

/// Every country and continent the game knows.
#[utoipa::path(get, path = "/api/countries", tag = "meta", responses((status = 200, body = Catalog)))]
pub(super) async fn countries() -> Json<Catalog> {
    Json(Catalog {
        countries: catalog::countries(),
        continents: catalog::CONTINENTS
            .iter()
            .map(|&(code, name)| Continent { code, name })
            .collect(),
    })
}
