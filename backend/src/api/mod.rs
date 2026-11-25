//! HTTP API. Everything lives under `/api` and speaks JSON, so the web app and the
//! mobile apps share one contract.

mod account;
mod additions;
mod celebrations;
mod discover;
mod error;
mod extract;
mod meta;
mod places;
mod refresh;
mod session;
mod sign_in;
mod unknown;
mod visits;

use std::path::Path as FsPath;

use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use std::collections::HashMap;

use chrono_tz::Tz;

use additions::{add, AddedPlace};
use discover::{zone, AreaQuery};
pub use error::ApiError;
use error::{provider_error, ErrorBody};
use extract::{ApiResult, CurrentSession, JsonBody, MaybeSession, MaybeUser, Path, Query, User};
use places::{complete, PlaceQuery};
use session::Session;

use crate::game::{self, Summary};
use crate::providers::{AreaInfo, Located};
use crate::push;
use crate::store;
use crate::{areas, limits, share, AppState};

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
        .routes(routes!(share_visit))
        .routes(routes!(share_profile, unshare_profile))
        .routes(routes!(push_key))
        .routes(routes!(subscribe))
        .routes(routes!(unsubscribe))
        .routes(routes!(search_areas))
        .routes(routes!(locate_area))
        .routes(routes!(suggest_areas))
        .routes(routes!(area_progress))
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
            page_or_gone(profile_page(state, headers, slug).await)
        }),
    )
    .route("/p/{slug}/card.png", get(profile_card))
    .route(
        "/c/{slug}",
        get(|state, headers, slug| async { page_or_gone(visit_page(state, headers, slug).await) }),
    )
    .route("/c/{slug}/card.png", get(visit_card))
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

/// A public link and what to say alongside it.
#[derive(Serialize, ToSchema)]
struct ShareLink {
    /// The public page, also the link preview.
    url: String,
    /// The card image: 1200 × 630 PNG, the map with a caption.
    image_url: String,
    /// A sentence to share with the link.
    text: String,
}

/// `https://mangiamundo.com`: where the request was sent, as the browser or app saw it.
fn origin(headers: &header::HeaderMap) -> String {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    let local = host.starts_with("localhost")
        || host.starts_with("127.0.0.1")
        || host.starts_with("10.0.2.2");
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .unwrap_or(if local { "http" } else { "https" });
    format!("{proto}://{host}")
}

fn host(headers: &header::HeaderMap) -> String {
    origin(headers)
        .split("://")
        .nth(1)
        .unwrap_or_default()
        .to_string()
}

/// Shares the user's progress in an area: makes their public page (the first time) or
/// moves it to this area, and returns its link. `DELETE` turns the page off.
#[utoipa::path(
    post, path = "/api/share", tag = "share", params(AreaQuery), security(("token" = [])),
    responses((status = 200, body = ShareLink), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn share_profile(
    State(state): State<AppState>,
    User(user): User,
    headers: header::HeaderMap,
    Query(query): Query<AreaQuery>,
) -> ApiResult<ShareLink> {
    let area = areas::resolve(&state, &query.area).await?;
    let slug = share::publish_profile(&state.db, &user, area.id(), state.now()).await?;
    let rows = store::area_rows(&state.db, area.id(), Some(&user)).await?;
    let today = state.today(zone(query.tz.as_deref()));
    let summary = game::summarize(&rows, store::lifetime(&state.db, &user, today).await?);
    let url = format!("{}/p/{slug}", origin(&headers));
    Ok(Json(ShareLink {
        image_url: format!("{url}/card.png"),
        url,
        text: share::profile_text(&summary, &area.boundary.info.name),
    }))
}

/// Turns the public page off: its link stops working. Sharing again makes a new one.
#[utoipa::path(
    delete, path = "/api/share", tag = "share", security(("token" = [])),
    responses((status = 204, description = "Off, or it was never on"), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn unshare_profile(
    State(state): State<AppState>,
    User(user): User,
) -> Result<StatusCode, ApiError> {
    share::unpublish_profile(&state.db, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Shares one check-in: a public page with the place, its food and the date. The same
/// link each time; it goes when the check-in is removed.
#[utoipa::path(
    post, path = "/api/visits/{visit_id}/share", tag = "share",
    params(("visit_id" = i64, Path, description = "A visit id from a place's `visits`.")),
    security(("token" = [])),
    responses((status = 200, body = ShareLink), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn share_visit(
    State(state): State<AppState>,
    User(user): User,
    headers: header::HeaderMap,
    Path(visit_id): Path<i64>,
) -> ApiResult<ShareLink> {
    let slug = share::share_visit(&state.db, &user, visit_id, state.now())
        .await?
        .ok_or(ApiError::NotFound("visit"))?;
    let visit = share::find_visit(&state.db, &slug)
        .await?
        .ok_or(ApiError::NotFound("visit"))?;
    let url = format!("{}/c/{slug}", origin(&headers));
    Ok(Json(ShareLink {
        image_url: format!("{url}/card.png"),
        url,
        text: format!("{} on Mangiamundo.", share::visit_headline(&visit)),
    }))
}

/// A public page's progress: the summary and the area's name.
async fn public_profile(state: &AppState, slug: &str) -> Result<(Summary, String), ApiError> {
    let (user, area_id) = share::find_profile(&state.db, slug)
        .await?
        .ok_or(ApiError::NotFound("page"))?;
    let area = store::find_area(&state.db, &area_id)
        .await?
        .ok_or(ApiError::NotFound("page"))?;
    let rows = store::area_rows(&state.db, &area_id, Some(&user)).await?;
    // ponytail: the viewer's zone is unknown here; UTC can be a day off for the streak.
    let today = state.today(Tz::UTC);
    let summary = game::summarize(&rows, store::lifetime(&state.db, &user, today).await?);
    Ok((summary, area.boundary.info.name))
}

fn html(body: String) -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
}

/// People open these links in a browser, so a page that's gone says so in HTML.
fn page_or_gone(result: Result<Response, ApiError>) -> Response {
    match result {
        Err(ApiError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            html(share::page(
                "This link is gone",
                "Whoever shared it turned it off.",
                "/icon.svg",
                "/",
                "<h1>This link is gone</h1>\n<p class=\"muted\">Whoever shared it turned it off or removed the check-in.</p>\n",
            )),
        )
            .into_response(),
        other => other.into_response(),
    }
}

fn png(bytes: Vec<u8>) -> Response {
    (
        // Not kept by caches: a card must stop showing once its share is turned off.
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response()
}

/// Draws a card off the async threads; it takes a few dozen milliseconds.
async fn render_card(
    statuses: HashMap<String, game::Status>,
    headline: String,
    caption: String,
) -> Result<Vec<u8>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let statuses: HashMap<&str, game::Status> =
            statuses.iter().map(|(c, s)| (c.as_str(), *s)).collect();
        share::card_png(&statuses, &headline, &caption)
    })
    .await
    .map_err(|e| ApiError::Internal(e.into()))?
    .map_err(ApiError::Internal)
}

async fn profile_page(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    Path(slug): Path<String>,
) -> Result<Response, ApiError> {
    let (summary, area) = public_profile(&state, &slug).await?;
    let url = format!("{}/p/{slug}", origin(&headers));
    Ok(html(share::page(
        &share::profile_headline(&summary),
        &share::profile_text(&summary, &area),
        &format!("{url}/card.png"),
        &url,
        &share::profile_body(&summary, &area),
    )))
}

async fn profile_card(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    Path(slug): Path<String>,
) -> Result<Response, ApiError> {
    let (summary, area) = public_profile(&state, &slug).await?;
    let statuses = summary
        .countries
        .iter()
        .map(|c| (c.code.clone(), c.status))
        .collect();
    let caption = format!(
        "{area} · Level {} · {}",
        summary.profile.level,
        host(&headers)
    );
    Ok(png(render_card(
        statuses,
        share::profile_headline(&summary),
        caption,
    )
    .await?))
}

async fn visit_page(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    Path(slug): Path<String>,
) -> Result<Response, ApiError> {
    let visit = share::find_visit(&state.db, &slug)
        .await?
        .ok_or(ApiError::NotFound("page"))?;
    let url = format!("{}/c/{slug}", origin(&headers));
    let headline = share::visit_headline(&visit);
    Ok(html(share::page(
        &headline,
        &format!("{headline}, {}.", share::pretty_date(&visit.date)),
        &format!("{url}/card.png"),
        &url,
        &share::visit_body(&visit),
    )))
}

async fn visit_card(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    Path(slug): Path<String>,
) -> Result<Response, ApiError> {
    let visit = share::find_visit(&state.db, &slug)
        .await?
        .ok_or(ApiError::NotFound("page"))?;
    // The check-in's countries light up gold, as freshly tasted.
    let statuses = visit
        .countries
        .iter()
        .map(|c| (c.clone(), game::Status::Discovered))
        .collect();
    let caption = format!("{} · {}", share::pretty_date(&visit.date), host(&headers));
    Ok(png(render_card(
        statuses,
        share::visit_headline(&visit),
        caption,
    )
    .await?))
}

#[derive(Serialize, ToSchema)]
struct PushKey {
    /// The VAPID public key browsers subscribe with.
    public_key: String,
}

/// The key for Web Push. A 404 when the server has push turned off.
#[utoipa::path(get, path = "/api/push/key", tag = "push", responses((status = 200, body = PushKey), (status = "default", description = "An error", body = ErrorBody)))]
async fn push_key(State(state): State<AppState>) -> ApiResult<PushKey> {
    let public_key = state.push_key.clone().ok_or(ApiError::NotFound("push"))?;
    Ok(Json(PushKey { public_key }))
}

/// A browser's `PushSubscription` (as from `subscription.toJSON()`), plus the user's area.
#[derive(Deserialize, ToSchema)]
struct SubscribeBody {
    endpoint: String,
    keys: SubscribeKeys,
    /// The area to send reminders for.
    area: String,
    /// IANA name. Reminders go out from 10:00 in this zone.
    timezone: String,
}

#[derive(Deserialize, ToSchema)]
struct SubscribeKeys {
    p256dh: String,
    auth: String,
}

/// Subscribes a browser to reminders.
#[utoipa::path(
    put, path = "/api/push/subscription", tag = "push", request_body = SubscribeBody,
    security(("token" = [])),
    responses((status = 204, description = "Subscribed"), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn subscribe(
    State(state): State<AppState>,
    User(user): User,
    JsonBody(body): JsonBody<SubscribeBody>,
) -> Result<StatusCode, ApiError> {
    if state.push_key.is_none() {
        return Err(ApiError::NotFound("push"));
    }
    if !body.endpoint.starts_with("https://") && !body.endpoint.starts_with("http://127.0.0.1") {
        return Err(ApiError::BadRequest("endpoint must be an https URL".into()));
    }
    if body.timezone.parse::<chrono_tz::Tz>().is_err() {
        return Err(ApiError::BadRequest("unknown time zone".into()));
    }
    let subscription = push::Subscription {
        endpoint: body.endpoint,
        p256dh: body.keys.p256dh,
        auth: body.keys.auth,
        area_id: Some(areas::resolve(&state, &body.area).await?.id().to_string()),
        timezone: body.timezone,
    };
    store::save_push_subscription(&state.db, &user, &subscription, state.now()).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
struct UnsubscribeBody {
    endpoint: String,
}

/// Unsubscribes a browser. A POST, because a DELETE with a body has no defined meaning
/// and some proxies drop it.
#[utoipa::path(
    post, path = "/api/push/subscription/delete", tag = "push", request_body = UnsubscribeBody,
    security(("token" = [])),
    responses((status = 204, description = "Unsubscribed"), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn unsubscribe(
    State(state): State<AppState>,
    User(user): User,
    JsonBody(body): JsonBody<UnsubscribeBody>,
) -> Result<StatusCode, ApiError> {
    store::remove_user_push_subscription(&state.db, &user, &body.endpoint).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct SearchQuery {
    /// What someone typed. Fewer than 2 characters returns nothing.
    q: String,
}

/// Cities and postal codes matching what someone typed.
///
/// For a search someone submits: `/api/areas/suggest` is the one to call while they type.
#[utoipa::path(
    get, path = "/api/areas", tag = "areas", params(SearchQuery),
    responses(
        (status = 200, body = Vec<AreaInfo>),
        (status = 429, description = "`too_many_requests`: this client searched for too many new things (more than 10 in a minute or 60 in an hour), or too many lookups are waiting", body = ErrorBody),
        (status = "default", description = "An error", body = ErrorBody),
    ),
)]
async fn search_areas(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Vec<AreaInfo>> {
    let q = query.q.trim();
    if q.chars().count() < 2 {
        return Ok(Json(Vec::new()));
    }
    state
        .areas
        .search(q)
        .await
        .map(Json)
        .map_err(provider_error)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct SuggestQuery {
    /// What someone typed so far. Fewer than 2 characters returns nothing.
    q: String,
    /// Where the user is, to rank nearby areas first.
    lat: Option<f64>,
    lng: Option<f64>,
}

/// Suggestions while someone types a city or postal code.
#[utoipa::path(
    get, path = "/api/areas/suggest", tag = "areas", params(SuggestQuery),
    responses((status = 200, body = Vec<AreaInfo>), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn suggest_areas(
    State(state): State<AppState>,
    Query(query): Query<SuggestQuery>,
) -> ApiResult<Vec<AreaInfo>> {
    let q = query.q.trim();
    if q.chars().count() < 2 {
        return Ok(Json(Vec::new()));
    }
    let near = query.lat.zip(query.lng);
    state
        .areas
        .suggest(q, near)
        .await
        .map(Json)
        .map_err(ApiError::Upstream)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct LocateQuery {
    lat: f64,
    lng: f64,
}

#[derive(Serialize, ToSchema)]
struct LoadProgress {
    /// 0 to 100.
    percent: u8,
    /// What is happening, e.g. "Looking for restaurants".
    text: String,
}

/// How far loading an area's restaurants has come, for a progress wheel while `/api/discover`
/// or `/api/places` waits on it. `null` when the area isn't loading.
#[utoipa::path(
    get, path = "/api/areas/{area_id}/progress", tag = "areas", params(("area_id" = String, Path, description = "An area id.")),
    responses((status = 200, body = Option<LoadProgress>), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn area_progress(
    State(state): State<AppState>,
    Path(area_id): Path<String>,
) -> Json<Option<LoadProgress>> {
    Json(
        state
            .loads
            .progress(&area_id)
            .map(|(percent, text)| LoadProgress {
                percent,
                text: text.into(),
            }),
    )
}

/// The city and postal code a point (usually the device's position) lies in.
#[utoipa::path(
    get, path = "/api/areas/locate", tag = "areas", params(LocateQuery),
    responses(
        (status = 200, body = Located),
        (status = 429, description = "`too_many_requests`: this client asked about too many new places (more than 5 in a minute or 30 in an hour), or too many lookups are waiting", body = ErrorBody),
        (status = "default", description = "An error", body = ErrorBody),
    ),
)]
async fn locate_area(
    State(state): State<AppState>,
    Query(query): Query<LocateQuery>,
) -> ApiResult<Located> {
    if !(-90.0..=90.0).contains(&query.lat) || !(-180.0..=180.0).contains(&query.lng) {
        return Err(ApiError::BadRequest("lat/lng out of range".into()));
    }
    state
        .areas
        .locate(query.lat, query.lng)
        .await
        .map(Json)
        .map_err(provider_error)
}
