//! HTTP API. Everything lives under `/api` and speaks JSON, so the web app and the
//! mobile apps share one contract.

mod account;
mod additions;
mod area_search;
mod celebrations;
mod discover;
mod error;
mod extract;
mod meta;
mod pages;
mod places;
mod refresh;
mod session;
mod sharing;
mod sign_in;
mod unknown;
mod visits;
mod web_push;

use std::path::Path as FsPath;

use axum::extract::Request;
use axum::http::header;
use axum::response::Response;
use axum::routing::{any, get};
use axum::{Json, Router};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use additions::{add, AddedPlace};
use discover::{zone, AreaQuery};
pub use error::ApiError;
use error::{provider_error, ErrorBody};
use extract::{ApiResult, CurrentSession, JsonBody, MaybeSession, MaybeUser, Path, Query, User};
use places::{complete, PlaceQuery};
use session::Session;
use sharing::{host, origin};

use crate::{limits, AppState};

/// The OpenAPI document's header. The paths come from the routes in [`api_router`].
#[derive(OpenApi)]
#[openapi(
    info(title = "Mangiamundo API", description = "Find restaurants from around the world near you, and collect the countries you have tasted."),
    modifiers(&BearerToken),
)]
struct ApiDoc;

/// Routes that need an account send `Authorization: Bearer <token>` from `POST /api/session`.
struct BearerToken;

impl utoipa::Modify for BearerToken {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "token",
            SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
        );
    }
}

/// The documented API routes and the OpenAPI document that describes them. A route
/// registered with `routes!` can't exist without being in the document.
fn api_router() -> (Router<AppState>, utoipa::openapi::OpenApi) {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(meta::health))
        .routes(routes!(session::create_session))
        .routes(routes!(session::restore_session))
        .routes(routes!(sign_in::auth_providers))
        .routes(routes!(sign_in::sign_in_apple))
        .routes(routes!(sign_in::sign_in_google))
        .routes(routes!(sign_in::send_email_code))
        .routes(routes!(sign_in::sign_in_email))
        .routes(routes!(account::get_account, account::delete_account))
        .routes(routes!(account::new_recovery_code))
        .routes(routes!(account::new_link_code))
        .routes(routes!(account::remove_device))
        .routes(routes!(meta::countries))
        .routes(routes!(places::nearest))
        .routes(routes!(discover::discover))
        .routes(routes!(places::places, additions::add_place))
        .routes(routes!(unknown::unknown_places))
        .routes(routes!(unknown::suggest_cuisine))
        .routes(routes!(unknown::no_cuisine))
        .routes(routes!(refresh::refresh_place))
        .routes(routes!(refresh::refresh_nearby))
        .routes(routes!(additions::additions))
        .routes(routes!(additions::mark_additions_seen))
        .routes(routes!(places::place))
        .routes(routes!(places::suggestion))
        .routes(routes!(celebrations::celebrations))
        .routes(routes!(celebrations::reminders))
        .routes(routes!(visits::report_closed))
        .routes(routes!(visits::add_visit))
        .routes(routes!(visits::remove_visit))
        .routes(routes!(sharing::share_visit))
        .routes(routes!(sharing::share_profile, sharing::unshare_profile))
        .routes(routes!(web_push::push_key))
        .routes(routes!(web_push::subscribe))
        .routes(routes!(web_push::unsubscribe))
        .routes(routes!(area_search::search_areas))
        .routes(routes!(area_search::locate_area))
        .routes(routes!(area_search::suggest_areas))
        .routes(routes!(area_search::area_progress))
        .split_for_parts()
}

pub fn router(state: AppState) -> Router {
    let (api, doc) = api_router();
    // The document itself and the catch-alls below are the only routes not in it.
    api.route(
        "/api/openapi.json",
        get(move || std::future::ready(Json(doc.clone()))),
    )
    // Public pages of shared progress and check-ins, with their card images.
    .route(
        "/p/{slug}",
        get(|state, headers, slug| async {
            pages::page_or_gone(pages::profile_page(state, headers, slug).await)
        }),
    )
    .route("/p/{slug}/card.png", get(pages::profile_card))
    .route(
        "/c/{slug}",
        get(|state, headers, slug| async {
            pages::page_or_gone(pages::visit_page(state, headers, slug).await)
        }),
    )
    .route("/c/{slug}/card.png", get(pages::visit_card))
    // Unknown API paths get a JSON 404 rather than the web app's index.html.
    .route("/api", any(not_found))
    .route("/api/", any(not_found))
    .route("/api/{*rest}", any(not_found))
    .method_not_allowed_fallback(|| async { ApiError::MethodNotAllowed })
    .layer(axum::middleware::from_fn(with_client))
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http())
    .with_state(state)
}

/// Who sent a request: the address Fly's proxy saw it come from, which the proxy puts in
/// `Fly-Client-IP` itself, whatever the client sent. An IPv6 address counts as its /64,
/// since one connection has all of those. Without the proxy (development, tests) everyone
/// is `local`.
fn client_of(headers: &header::HeaderMap) -> String {
    let ip = headers
        .get("fly-client-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<std::net::IpAddr>().ok());
    match ip {
        Some(std::net::IpAddr::V4(ip)) => ip.to_string(),
        Some(std::net::IpAddr::V6(ip)) => {
            let [a, b, c, d, ..] = ip.segments();
            format!("{a:x}:{b:x}:{c:x}:{d:x}::/64")
        }
        None => "local".into(),
    }
}

/// Handles the request with its sender known to [`client`].
async fn with_client(request: Request, next: axum::middleware::Next) -> Response {
    limits::as_client(client_of(request.headers()), next.run(request)).await
}

/// Serves the web app from `dir` next to the API. It is a single-page app, so unknown
/// paths outside `/api` fall back to index.html.
pub fn with_web_app(app: Router, dir: &FsPath) -> Router {
    app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html"))))
}

async fn not_found() -> ApiError {
    ApiError::NotFound("API endpoint")
}
