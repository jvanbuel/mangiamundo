//! HTTP API. Everything lives under `/api` and speaks JSON, so the web app and the
//! mobile apps share one contract.

use std::path::Path as FsPath;

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, OptionalFromRequest, Request, State};
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::NaiveDate;
use chrono_tz::Tz;

use crate::calendar::{self, Celebration};
use crate::game::{self, Lifetime, PlaceRow, Summary};
use crate::providers::{AreaInfo, Located, Lookup};
use crate::store::{self, Area, HoursStatus, PlaceView, Visit};
use crate::{account, catalog, geo, hours, share, AppState};

/// A check-in counts as verified when made within this distance of the place.
const VERIFY_DISTANCE_M: f64 = 250.0;

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
        .routes(routes!(health))
        .routes(routes!(create_session))
        .routes(routes!(restore_session))
        .routes(routes!(get_account, delete_account))
        .routes(routes!(new_recovery_code))
        .routes(routes!(new_link_code))
        .routes(routes!(remove_device))
        .routes(routes!(countries))
        .routes(routes!(discover))
        .routes(routes!(places, add_place))
        .routes(routes!(refresh_place))
        .routes(routes!(refresh_nearby))
        .routes(routes!(additions))
        .routes(routes!(mark_additions_seen))
        .routes(routes!(place))
        .routes(routes!(celebrations))
        .routes(routes!(report_closed))
        .routes(routes!(add_visit))
        .routes(routes!(remove_visit))
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
    .route("/p/{slug}", get(|state, headers, slug| async { page_or_gone(profile_page(state, headers, slug).await) }))
    .route("/p/{slug}/card.png", get(profile_card))
    .route("/c/{slug}", get(|state, headers, slug| async { page_or_gone(visit_page(state, headers, slug).await) }))
    .route("/c/{slug}/card.png", get(visit_card))
    // Unknown API paths get a JSON 404 rather than the web app's index.html.
    .route("/api", any(not_found))
    .route("/api/", any(not_found))
    .route("/api/{*rest}", any(not_found))
    .method_not_allowed_fallback(|| async { ApiError::MethodNotAllowed })
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http())
    .with_state(state)
}

#[derive(Serialize, ToSchema)]
struct Health {
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
async fn health() -> Json<Health> {
    Json(Health {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
        commit: option_env!("GIT_COMMIT"),
    })
}

/// Serves the web app from `dir` next to the API. It is a single-page app, so unknown
/// paths outside `/api` fall back to index.html.
pub fn with_web_app(app: Router, dir: &FsPath) -> Router {
    app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html"))))
}

async fn not_found() -> ApiError {
    ApiError::NotFound("API endpoint")
}

/// Every error response, whatever the status.
#[derive(Serialize, ToSchema)]
struct ErrorBody {
    /// For people. It may change, so clients branch on `code` instead.
    error: String,
    code: ErrorCode,
}

/// What went wrong, for clients that react to a specific case:
///
/// - `bad_request` (400, 413, 415, 422): invalid or missing parameters, or a malformed,
///   oversized (over 2 MiB) or non-JSON body.
/// - `unauthorized` (401): a missing or unknown session token.
/// - `not_found` (404): an unknown place, area, device, code or `/api` path.
/// - `method_not_allowed` (405): the path exists, but not with this method.
/// - `too_many_requests` (429): a limit was reached, such as places added per day.
/// - `upstream` (502): the places or areas provider failed.
/// - `internal` (500): anything else, such as a database error.
#[derive(Serialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    BadRequest,
    Unauthorized,
    NotFound,
    MethodNotAllowed,
    TooManyRequests,
    Upstream,
    Internal,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    /// A path, query string or body that axum could not parse, with axum's status
    /// (400, 413, 415 or 422).
    #[error("{1}")]
    Rejected(StatusCode, String),
    #[error("method not allowed")]
    MethodNotAllowed,
    #[error("missing or invalid session token")]
    Unauthorized,
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    TooManyRequests(&'static str),
    #[error("the places provider is unavailable")]
    Upstream(#[source] anyhow::Error),
    #[error("internal error")]
    Database(#[from] sqlx::Error),
    #[error("internal error")]
    Internal(#[source] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, ErrorCode::BadRequest),
            ApiError::Rejected(status, _) => (*status, ErrorCode::BadRequest),
            ApiError::MethodNotAllowed => {
                (StatusCode::METHOD_NOT_ALLOWED, ErrorCode::MethodNotAllowed)
            }
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, ErrorCode::Unauthorized),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, ErrorCode::NotFound),
            ApiError::TooManyRequests(_) => {
                (StatusCode::TOO_MANY_REQUESTS, ErrorCode::TooManyRequests)
            }
            ApiError::Upstream(error) => {
                tracing::error!(error = format!("{error:#}"), "provider request failed");
                (StatusCode::BAD_GATEWAY, ErrorCode::Upstream)
            }
            ApiError::Database(error) => {
                tracing::error!(%error, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::Internal)
            }
            ApiError::Internal(error) => {
                tracing::error!(error = format!("{error:#}"), "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::Internal)
            }
        };
        (
            status,
            Json(ErrorBody {
                error: self.to_string(),
                code,
            }),
        )
            .into_response()
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        ApiError::Rejected(rejection.status(), rejection.body_text())
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        ApiError::Rejected(rejection.status(), rejection.body_text())
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        ApiError::Rejected(rejection.status(), rejection.body_text())
    }
}

/// axum's `Query`, but a malformed query string is a JSON error like any other.
struct Query<T>(T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequestParts<S> for Query<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        let axum::extract::Query(value) =
            axum::extract::Query::from_request_parts(parts, state).await?;
        Ok(Query(value))
    }
}

/// axum's `Path`, but a path segment that does not decode is a JSON error like any other.
struct Path<T>(T);

impl<S: Send + Sync, T: DeserializeOwned + Send> FromRequestParts<S> for Path<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        let axum::extract::Path(value) =
            axum::extract::Path::from_request_parts(parts, state).await?;
        Ok(Path(value))
    }
}

/// axum's `Json` for request bodies, but a bad body is a JSON error like any other.
struct JsonBody<T>(T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for JsonBody<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, ApiError> {
        let Json(value) = <Json<T> as FromRequest<S>>::from_request(request, state).await?;
        Ok(JsonBody(value))
    }
}

/// No body (or no JSON content type) is `None`; a malformed JSON body is still an error.
impl<S: Send + Sync, T: DeserializeOwned> OptionalFromRequest<S> for JsonBody<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Option<Self>, ApiError> {
        let value = <Json<T> as OptionalFromRequest<S>>::from_request(request, state).await?;
        Ok(value.map(|Json(value)| JsonBody(value)))
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

/// The session behind the `Authorization: Bearer <token>` header, if there is one.
struct MaybeSession(Option<account::Session>);
/// Like [`MaybeSession`] but rejects anonymous requests.
struct CurrentSession(account::Session);
/// The user behind the `Authorization: Bearer <token>` header, if there is one.
struct MaybeUser(Option<String>);
/// Like [`MaybeUser`] but rejects anonymous requests.
struct User(String);

impl FromRequestParts<AppState> for MaybeSession {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let Some(value) = parts.headers.get(header::AUTHORIZATION) else {
            return Ok(MaybeSession(None));
        };
        let token = value
            .to_str()
            .ok()
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        match account::session_for_token(&state.db, token, state.now()).await? {
            Some(session) => Ok(MaybeSession(Some(session))),
            None => Err(ApiError::Unauthorized),
        }
    }
}

impl FromRequestParts<AppState> for CurrentSession {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        match MaybeSession::from_request_parts(parts, state).await? {
            MaybeSession(Some(session)) => Ok(CurrentSession(session)),
            MaybeSession(None) => Err(ApiError::Unauthorized),
        }
    }
}

impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let MaybeSession(session) = MaybeSession::from_request_parts(parts, state).await?;
        Ok(MaybeUser(session.map(|s| s.user)))
    }
}

impl FromRequestParts<AppState> for User {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let CurrentSession(session) = CurrentSession::from_request_parts(parts, state).await?;
        Ok(User(session.user))
    }
}

#[derive(Serialize, ToSchema)]
struct Session {
    /// Send it as `Authorization: Bearer <token>`.
    token: String,
}

#[derive(Deserialize, ToSchema)]
struct NewSession {
    /// What the app calls this device in the account's device list, such as "iPhone",
    /// "Android" or "Web". At most 60 characters are kept.
    #[serde(default)]
    device: Option<String>,
}

/// Creates an anonymous account. A device asks for a token once and keeps it.
#[utoipa::path(
    post, path = "/api/session", tag = "account",
    request_body(content = Option<NewSession>, description = "Optional."),
    responses((status = 200, body = Session), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn create_session(
    State(state): State<AppState>,
    body: Option<JsonBody<NewSession>>,
) -> ApiResult<Session> {
    let device = body.and_then(|JsonBody(b)| b.device);
    let token = account::create_user(&state.db, device.as_deref(), state.now()).await?;
    Ok(Json(Session { token }))
}

#[derive(Deserialize, ToSchema)]
struct Restore {
    /// A recovery code (`XXXX-XXXX-XXXX-XXXX-XXXX`) or a link code (`XXXX-XXXX`). Case,
    /// spaces and dashes don't matter.
    code: String,
    /// As in `POST /api/session`.
    #[serde(default)]
    device: Option<String>,
}

/// Signs this device in to the account a recovery code or link code belongs to.
///
/// Sent with the device's current token, that session ends, and when it belonged to
/// another account, that account's check-ins, added places and the rest move into the
/// restored account, which then replaces it. A link code works once.
#[utoipa::path(
    post, path = "/api/session/restore", tag = "account", request_body = Restore,
    security((), ("token" = [])),
    responses(
        (status = 200, body = Session),
        (status = 404, description = "`not_found`: an unknown, used or expired code", body = ErrorBody),
        (status = "default", description = "An error", body = ErrorBody),
    ),
)]
async fn restore_session(
    State(state): State<AppState>,
    MaybeSession(current): MaybeSession,
    JsonBody(body): JsonBody<Restore>,
) -> ApiResult<Session> {
    let token = account::redeem(
        &state.db,
        &body.code,
        current.as_ref(),
        body.device.as_deref(),
        state.now(),
    )
    .await?
    .ok_or(ApiError::NotFound("code"))?;
    Ok(Json(Session { token }))
}

/// The account: whether it has a recovery code, and the devices signed in to it.
#[utoipa::path(
    get, path = "/api/account", tag = "account", security(("token" = [])),
    responses((status = 200, body = account::Account), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn get_account(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
) -> ApiResult<account::Account> {
    Ok(Json(account::account(&state.db, &session).await?))
}

/// Deletes the account and everything in it: check-ins, added places, closed reports,
/// reminders, and the sessions of every device. It can't be undone.
#[utoipa::path(
    delete, path = "/api/account", tag = "account", security(("token" = [])),
    responses((status = 204, description = "Deleted"), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn delete_account(
    State(state): State<AppState>,
    User(user): User,
) -> Result<StatusCode, ApiError> {
    account::delete(&state.db, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
struct RecoveryCode {
    /// `XXXX-XXXX-XXXX-XXXX-XXXX`. Shown this once: the server keeps only a hash.
    code: String,
}

/// Makes a new recovery code, which restores the account on any device through
/// `POST /api/session/restore`. The previous code stops working.
#[utoipa::path(
    post, path = "/api/account/recovery-code", tag = "account", security(("token" = [])),
    responses((status = 200, body = RecoveryCode), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn new_recovery_code(
    State(state): State<AppState>,
    User(user): User,
) -> ApiResult<RecoveryCode> {
    Ok(Json(RecoveryCode {
        code: account::new_recovery_code(&state.db, &user, state.now()).await?,
    }))
}

#[derive(Serialize, ToSchema)]
struct LinkCode {
    /// `XXXX-XXXX`, to enter on the other device. It works once.
    code: String,
    expires_at: i64,
}

/// Makes a code that signs another device in to this account, through
/// `POST /api/session/restore`, within 10 minutes.
#[utoipa::path(
    post, path = "/api/account/link-code", tag = "account", security(("token" = [])),
    responses(
        (status = 200, body = LinkCode),
        (status = 429, description = "`too_many_requests`: more than 10 codes in an hour", body = ErrorBody),
        (status = "default", description = "An error", body = ErrorBody),
    ),
)]
async fn new_link_code(State(state): State<AppState>, User(user): User) -> ApiResult<LinkCode> {
    let now = state.now();
    if !state.limits.allow(&user, "link_code", 10, 3600, now) {
        return Err(ApiError::TooManyRequests(
            "too many link codes, try again later",
        ));
    }
    let (code, expires_at) = account::new_link_code(&state.db, &user, now).await?;
    Ok(Json(LinkCode { code, expires_at }))
}

/// Signs a device out of the account. Signing out the current device ends this session.
#[utoipa::path(
    delete, path = "/api/account/devices/{device_id}", tag = "account",
    params(("device_id" = String, Path, description = "A device id from `GET /api/account`.")),
    security(("token" = [])),
    responses((status = 204, description = "Signed out"), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn remove_device(
    State(state): State<AppState>,
    User(user): User,
    Path(device_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if !account::remove_device(&state.db, &user, &device_id).await? {
        return Err(ApiError::NotFound("device"));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
struct Catalog {
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
async fn countries() -> Json<Catalog> {
    Json(Catalog {
        countries: catalog::countries(),
        continents: catalog::CONTINENTS
            .iter()
            .map(|&(code, name)| Continent { code, name })
            .collect(),
    })
}

/// The largest area served, in degrees of latitude and longitude: a big city fits, a
/// region or a country (which would make for a huge Overpass query) doesn't.
const MAX_AREA_DEGREES: f64 = 1.5;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct AreaQuery {
    /// An area id from `/api/areas` or `/api/areas/locate`.
    area: String,
    /// The user's IANA time zone (e.g. `Asia/Tokyo`), which decides which day "today"
    /// is for event and celebration dates. UTC when missing or unknown.
    tz: Option<String>,
}

/// Parses the optional `tz` parameter. An unknown zone (a browser can report one the
/// tz database here lacks) falls back to UTC rather than failing the whole screen.
fn zone(tz: Option<&str>) -> Tz {
    match tz {
        None | Some("") => Tz::UTC,
        Some(name) => name.parse().unwrap_or_else(|_| {
            tracing::warn!(tz = name, "unknown time zone, using UTC");
            Tz::UTC
        }),
    }
}

fn valid_area_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The step an area's fetch is in: it runs from `from` to `to` percent and should take
/// about `expected_secs`.
struct LoadStep {
    from: f64,
    to: f64,
    text: &'static str,
    started: Instant,
    expected_secs: f64,
}

/// An area being fetched: how many requests are fetching it, and the latest step.
#[derive(Default)]
struct Load {
    running: usize,
    step: Option<LoadStep>,
}

/// Recent requests per user and kind, for rate limits. In memory: the server runs as one
/// machine, and a restart forgetting them is harmless.
#[derive(Clone, Default)]
pub struct Limits(Arc<Mutex<HashMap<LimitKey, Vec<i64>>>>);

/// A user id and the kind of request.
type LimitKey = (String, &'static str);

impl Limits {
    /// Counts the request and returns true when `user` made fewer than `max` requests of this
    /// kind in the `window` seconds before `now`.
    fn allow(&self, user: &str, kind: &'static str, max: usize, window: i64, now: i64) -> bool {
        let mut all = self.0.lock().unwrap();
        // ponytail: entries of users who stopped asking stay until restart; a few bytes each.
        let times = all.entry((user.to_string(), kind)).or_default();
        times.retain(|&at| at > now - window);
        let allowed = times.len() < max;
        if allowed {
            times.push(now);
        }
        allowed
    }
}

/// Areas being fetched right now, by area id. In memory: the server runs as one machine.
#[derive(Clone, Default)]
pub struct Loads(Arc<Mutex<HashMap<String, Load>>>);

impl Loads {
    /// Counts a fetch of the area until the returned guard drops, also on an error.
    fn begin<'a>(&'a self, id: &'a str) -> LoadDone<'a> {
        self.0
            .lock()
            .unwrap()
            .entry(id.to_string())
            .or_default()
            .running += 1;
        LoadDone(self, id)
    }

    fn step(&self, id: &str, from: f64, to: f64, text: &'static str, expected_secs: f64) {
        if let Some(load) = self.0.lock().unwrap().get_mut(id) {
            load.step = Some(LoadStep {
                from,
                to,
                text,
                started: Instant::now(),
                expected_secs,
            });
        }
    }

    /// Starts a new stretch of the current step from where it is now, with a new text.
    fn restart(&self, id: &str, text: &'static str, expected_secs: f64) {
        if let Some((percent, _)) = self.progress(id) {
            self.step(id, f64::from(percent), 95.0, text, expected_secs);
        }
    }

    /// The percentage and what is happening, or None when the area isn't loading.
    fn progress(&self, id: &str) -> Option<(u8, &'static str)> {
        let loads = self.0.lock().unwrap();
        let step = loads.get(id)?.step.as_ref()?;
        let percent = estimate(
            step.from,
            step.to,
            step.started.elapsed().as_secs_f64(),
            step.expected_secs,
        );
        Some((percent as u8, step.text))
    }
}

/// Ends one fetch of an area; the progress goes when the last concurrent fetch ends.
struct LoadDone<'a>(&'a Loads, &'a str);

impl Drop for LoadDone<'_> {
    fn drop(&mut self) {
        let mut loads = self.0 .0.lock().unwrap();
        if let Some(load) = loads.get_mut(self.1) {
            load.running -= 1;
            if load.running == 0 {
                loads.remove(self.1);
            }
        }
    }
}

/// Progress within a step that reports none: 63% of the way after the expected time, 95%
/// after three times that, and never the end, which only the next step reaches.
fn estimate(from: f64, to: f64, elapsed_secs: f64, expected_secs: f64) -> f64 {
    from + (to - from) * (1.0 - (-elapsed_secs / expected_secs.max(0.1)).exp())
}

/// Returns the cached area, asking the provider when the cache is missing or stale. A
/// stale cache is still used when the provider is down.
async fn resolve_area(state: &AppState, id: &str) -> Result<Area, ApiError> {
    if !valid_area_id(id) {
        return Err(ApiError::BadRequest("invalid area".into()));
    }
    let cached = store::find_area(&state.db, id).await?;
    if let Some(area) = &cached {
        if state.now() - area.fetched_at < state.cache_ttl_secs {
            return Ok(area.clone());
        }
    }

    let _done = state.loads.begin(id);
    let boundary = match &cached {
        // The outline rarely changes; keep it for refreshes.
        Some(area) => area.boundary.clone(),
        None => {
            state.loads.step(id, 0.0, 15.0, "Finding the area", 2.0);
            let boundary = state
                .areas
                .boundary(id)
                .await
                .map_err(ApiError::Upstream)?
                .ok_or(ApiError::NotFound("area"))?;
            let b = boundary.shape.bbox();
            if b.north - b.south > MAX_AREA_DEGREES || b.east - b.west > MAX_AREA_DEGREES {
                return Err(ApiError::BadRequest(format!(
                    "{} is too large; pick a postal code or a smaller town",
                    boundary.info.name
                )));
            }
            boundary
        }
    };
    let expected = state.places.expected_secs(&boundary);
    state
        .loads
        .step(id, 15.0, 95.0, "Looking for restaurants", expected);
    // Another server starts the estimate over from where the wheel is.
    let next_server = || state.loads.restart(id, "Trying another server", expected);
    match state.places.search(&boundary, &next_server).await {
        Ok(found) => {
            state.loads.step(id, 95.0, 100.0, "Counting countries", 0.5);
            let area = Area {
                boundary,
                provider: state.places.name().into(),
                fetched_at: state.now(),
            };
            store::save_area(&state.db, &area, &found).await?;
            tracing::info!(area = area.id(), places = found.len(), "area refreshed");
            Ok(area)
        }
        Err(error) => match cached {
            Some(area) => {
                tracing::warn!(
                    error = format!("{error:#}"),
                    "provider failed, serving stale area"
                );
                Ok(area)
            }
            None => Err(ApiError::Upstream(error)),
        },
    }
}

/// An area as clients see it.
#[derive(Serialize, ToSchema)]
struct AreaView {
    #[serde(flatten)]
    info: AreaInfo,
    fetched_at: i64,
}

impl From<&Area> for AreaView {
    fn from(area: &Area) -> Self {
        AreaView {
            info: area.boundary.info.clone(),
            fetched_at: area.fetched_at,
        }
    }
}

#[derive(Serialize, ToSchema)]
struct Discover {
    area: AreaView,
    #[serde(flatten)]
    summary: Summary,
    events: Vec<EventProgress>,
}

#[derive(Serialize, PartialEq, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
enum EventStatus {
    Upcoming,
    Active,
    Ended,
}

/// A time-limited challenge and how far the user got.
#[derive(Serialize, ToSchema)]
struct EventProgress {
    #[serde(flatten)]
    event: calendar::Event,
    status: EventStatus,
    progress: u32,
    earned: bool,
    /// Places in the area that count towards the event.
    places_nearby: u32,
}

/// How many places each country has in an area.
fn places_per_country(rows: &[PlaceRow]) -> HashMap<&str, u32> {
    let mut counts = HashMap::new();
    for row in rows {
        *counts.entry(row.country.as_str()).or_default() += 1;
    }
    counts
}

/// Events to show: upcoming within a month, running, or finished and earned.
async fn event_progress(
    state: &AppState,
    user: Option<&str>,
    rows: &[PlaceRow],
    zone: Tz,
) -> Result<Vec<EventProgress>, ApiError> {
    let today = state.today(zone);
    let counts = places_per_country(rows);
    let mut out = Vec::new();
    for event in calendar::relevant_events(today, 30) {
        let progress = match user {
            Some(user) => {
                store::visits_between(&state.db, user, &event.countries, event.starts, event.ends)
                    .await?
            }
            None => 0,
        };
        let earned = progress >= event.target;
        let status = if today < event.starts {
            EventStatus::Upcoming
        } else if today <= event.ends {
            EventStatus::Active
        } else {
            EventStatus::Ended
        };
        if status == EventStatus::Ended && !earned {
            continue;
        }
        out.push(EventProgress {
            places_nearby: event
                .countries
                .iter()
                .map(|c| counts.get(c.as_str()).copied().unwrap_or(0))
                .sum(),
            event: event.clone(),
            status,
            progress: progress.min(event.target),
            earned,
        });
    }
    Ok(out)
}

/// Countries available in the area, progress, badges, profile and events.
#[utoipa::path(
    get, path = "/api/discover", tag = "game",
    params(AreaQuery),
    // Anonymous requests work too; an unknown token is still a 401.
    security((), ("token" = [])),
    responses((status = 200, body = Discover), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn discover(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Query(query): Query<AreaQuery>,
) -> ApiResult<Discover> {
    let zone = zone(query.tz.as_deref());
    let area = resolve_area(&state, &query.area).await?;
    let rows = store::area_rows(&state.db, area.id(), user.as_deref()).await?;
    let lifetime = match &user {
        Some(user) => store::lifetime(&state.db, user).await?,
        None => Lifetime::default(),
    };
    let events = event_progress(&state, user.as_deref(), &rows, zone).await?;
    Ok(Json(Discover {
        summary: game::summarize(&rows, lifetime),
        area: (&area).into(),
        events,
    }))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct CelebrationsQuery {
    /// As for `/api/discover`.
    area: String,
    /// How many days ahead to look: 0 to 366, 60 when missing.
    days: Option<i64>,
    /// As for `/api/discover`.
    tz: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct NearbyCountry {
    code: String,
    places: u32,
}

#[derive(Serialize, ToSchema)]
struct CelebrationView {
    #[serde(flatten)]
    celebration: Celebration,
    /// The celebration's countries that have places in the area.
    nearby: Vec<NearbyCountry>,
    /// Notification text for the first day, for apps that schedule reminders themselves.
    reminder: crate::push::Notification,
}

/// Celebrations coming up for countries whose food can be found in the area, with
/// the reminder text.
#[utoipa::path(
    get, path = "/api/celebrations", tag = "calendar", params(CelebrationsQuery),
    responses((status = 200, body = Vec<CelebrationView>), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn celebrations(
    State(state): State<AppState>,
    Query(query): Query<CelebrationsQuery>,
) -> ApiResult<Vec<CelebrationView>> {
    let zone = zone(query.tz.as_deref());
    let area = resolve_area(&state, &query.area).await?;
    let rows = store::area_rows(&state.db, area.id(), None).await?;
    let counts = places_per_country(&rows);
    let days = query.days.unwrap_or(60).clamp(0, 366);
    Ok(Json(nearby_celebrations(state.today(zone), days, &counts)))
}

fn nearby_celebrations(
    today: NaiveDate,
    days: i64,
    counts: &HashMap<&str, u32>,
) -> Vec<CelebrationView> {
    calendar::upcoming(today, days)
        .into_iter()
        .filter_map(|celebration| {
            let nearby: Vec<NearbyCountry> = celebration
                .countries
                .iter()
                .filter_map(|code| {
                    Some(NearbyCountry {
                        code: code.clone(),
                        places: *counts.get(code.as_str())?,
                    })
                })
                .collect();
            let owned: HashMap<String, u32> =
                nearby.iter().map(|n| (n.code.clone(), n.places)).collect();
            let reminder = crate::push::celebration_notification(&celebration, &owned)?;
            Some(CelebrationView {
                celebration,
                nearby,
                reminder,
            })
        })
        .collect()
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct PlacesQuery {
    /// As for `/api/discover`.
    area: String,
    /// Only places serving this country's food (ISO 3166-1 alpha-2).
    country: Option<String>,
    /// Where the user is, for distances. Without it they are measured from the area's centre.
    lat: Option<f64>,
    lng: Option<f64>,
}

/// Places in the area: open first, then closed, then unknown hours, alphabetical
/// within each group.
#[utoipa::path(
    get, path = "/api/places", tag = "game", params(PlacesQuery),
    security((), ("token" = [])),
    responses((status = 200, body = Vec<PlaceView>), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn places(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Query(query): Query<PlacesQuery>,
) -> ApiResult<Vec<PlaceView>> {
    let area = resolve_area(&state, &query.area).await?;
    let country = query.country.map(|c| c.to_uppercase());
    let mut places =
        store::area_places(&state.db, area.id(), country.as_deref(), user.as_deref()).await?;
    let info = &area.boundary.info;
    let (from_lat, from_lng) = match (query.lat, query.lng) {
        (Some(lat), Some(lng)) => (lat, lng),
        _ => (info.lat, info.lng),
    };
    let now = state.now();
    for place in &mut places {
        complete(place, from_lat, from_lng, now);
    }
    // Open first, then closed, then unknown hours; alphabetical within each group.
    places.sort_by_cached_key(|p| (group(p), p.name.to_lowercase()));
    Ok(Json(places))
}

/// Fills in what the store leaves empty: the distance, whether the place is open now and
/// its OSM page.
fn complete(place: &mut PlaceView, from_lat: f64, from_lng: f64, now: i64) {
    place.osm_url = osm_link(&place.id);
    place.distance_m = geo::distance_m(from_lat, from_lng, place.lat, place.lng).round() as u32;
    if let Some(h) = &mut place.hours {
        h.status = hours::status(&h.raw, place.lat, place.lng, now)
            .map(|(open, text)| HoursStatus { open, text });
    }
}

fn group(place: &PlaceView) -> u8 {
    match place.hours.as_ref().and_then(|h| h.status.as_ref()) {
        Some(HoursStatus { open: true, .. }) => 0,
        Some(HoursStatus { open: false, .. }) => 1,
        None => 2,
    }
}

/// Places a user may add per day, so the app never floods OpenStreetMap with notes.
const PLACES_PER_DAY: i64 = 5;

/// A restaurant missing from the map.
#[derive(Deserialize, ToSchema)]
struct AddPlace {
    name: String,
    /// The country whose food it serves (ISO 3166-1 alpha-2), one from `/api/countries`
    /// with a cuisine.
    country: String,
    lat: f64,
    lng: f64,
    address: Option<String>,
    /// An `http` or `https` URL.
    website: Option<String>,
    /// `false` when the user adds it to OpenStreetMap themselves (in CoMaps or Organic Maps),
    /// so no note asks mappers to. Defaults to `true`.
    note: Option<bool>,
}

/// Trimmed, `None` when empty; an error past `max` characters.
fn text(value: Option<String>, field: &str, max: usize) -> Result<Option<String>, ApiError> {
    let value = value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    if value.as_ref().is_some_and(|v| v.chars().count() > max) {
        return Err(ApiError::BadRequest(format!(
            "{field} is longer than {max} characters"
        )));
    }
    Ok(value)
}

/// The place just added, and what it will earn.
#[derive(Serialize, ToSchema)]
struct AddedPlace {
    #[serde(flatten)]
    place: PlaceView,
    reward: Reward,
}

/// XP for an added restaurant, granted once OpenStreetMap has it.
#[derive(Serialize, ToSchema)]
struct Reward {
    xp: u32,
    bonus: game::Bonus,
    /// The city the bonus was judged in. `null` when no city around it was cached.
    #[schema(required = true)]
    area_name: Option<String>,
}

/// Adds a restaurant missing from the map. It shows up right away, but only for whoever
/// added it, and an OpenStreetMap note asks local mappers to add it there for everyone.
/// At most 5 a day. Once the import finds it on OpenStreetMap it earns up to `reward.xp`:
/// less when an earlier addition in the area claimed the same bonus, and nothing when
/// someone else added the same place first.
#[utoipa::path(
    post, path = "/api/places", tag = "game", request_body = AddPlace,
    security(("token" = [])),
    responses((status = 200, body = AddedPlace), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn add_place(
    State(state): State<AppState>,
    User(user): User,
    JsonBody(body): JsonBody<AddPlace>,
) -> ApiResult<AddedPlace> {
    let name = text(Some(body.name), "name", 100)?
        .ok_or(ApiError::BadRequest("name is missing".into()))?;
    let address = text(body.address, "address", 200)?;
    let url = text(body.website, "website", 300)?;
    if url
        .as_ref()
        .is_some_and(|u| !u.starts_with("https://") && !u.starts_with("http://"))
    {
        return Err(ApiError::BadRequest(
            "website must be an http or https URL".into(),
        ));
    }
    if !(-90.0..=90.0).contains(&body.lat) || !(-180.0..=180.0).contains(&body.lng) {
        return Err(ApiError::BadRequest("lat/lng out of range".into()));
    }
    let code = body.country.to_uppercase();
    let (country, cuisine) = catalog::country(&code)
        .zip(catalog::cuisine_tag(&code))
        .ok_or(ApiError::BadRequest("unknown country".into()))?;
    let now = state.now();
    let new = store::NewPlace {
        name,
        lat: body.lat,
        lng: body.lng,
        address,
        url,
        country: &country.code,
        cuisine,
    };
    let added = store::add_place(&state.db, &user, &new, now, PLACES_PER_DAY)
        .await?
        .ok_or(ApiError::TooManyRequests(
            "you have added 5 places today; try again tomorrow",
        ))?;
    let reward = Reward {
        xp: added.bonus.xp(),
        bonus: added.bonus,
        area_name: added.area_name,
    };
    let id = added.id;

    if let Some(notes) = state
        .osm_notes_url
        .clone()
        .filter(|_| body.note != Some(false))
    {
        let (db, id, token) = (state.db.clone(), id.clone(), state.osm_notes_token.clone());
        let text = missing_place_note(&new);
        tokio::spawn(async move {
            if let Some(note) = post_osm_note(notes, token, new.lat, new.lng, text).await {
                if let Err(error) = store::set_osm_note(&db, &id, note).await {
                    tracing::warn!(%error, "could not keep the osm note id");
                }
            }
        });
    }
    let mut place = store::place(&state.db, &id, Some(&user))
        .await?
        .ok_or(ApiError::NotFound("place"))?;
    let (lat, lng) = (place.lat, place.lng);
    complete(&mut place, lat, lng, now);
    Ok(Json(AddedPlace { place, reward }))
}

/// The restaurants the user added, newest first, with their rewards. Those confirmed since
/// the user was last told have `seen: false`, for a "now on the map" message.
#[utoipa::path(
    get, path = "/api/additions", tag = "game",
    security(("token" = [])),
    responses((status = 200, body = Vec<store::Addition>), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn additions(
    State(state): State<AppState>,
    User(user): User,
) -> ApiResult<Vec<store::Addition>> {
    Ok(Json(store::additions(&state.db, &user).await?))
}

/// Additions the user was told about.
#[derive(Deserialize, ToSchema)]
struct SeenAdditions {
    /// `id`s from `/api/additions`. At most 100.
    ids: Vec<String>,
}

/// The user was told about these confirmed additions: they come back with `seen: true`.
/// Pending ones, and ones not listed, are left alone.
#[utoipa::path(
    put, path = "/api/additions/seen", tag = "game", request_body = SeenAdditions,
    security(("token" = [])),
    responses((status = 204, description = "Marked"), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn mark_additions_seen(
    State(state): State<AppState>,
    User(user): User,
    JsonBody(body): JsonBody<SeenAdditions>,
) -> Result<StatusCode, ApiError> {
    if body.ids.len() > 100 {
        return Err(ApiError::BadRequest("at most 100 ids".into()));
    }
    store::mark_additions_seen(&state.db, &user, &body.ids).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// An OSM note mappers can act on: the tags to add, ready to copy.
fn missing_place_note(place: &store::NewPlace) -> String {
    let mut text = format!(
        "A Mangiamundo user who knows this place says a restaurant is missing here:\n\n\
         amenity=restaurant\nname={}\ncuisine={}\n",
        place.name, place.cuisine
    );
    if let Some(url) = &place.url {
        text += &format!("website={url}\n");
    }
    if let Some(address) = &place.address {
        text += &format!("\nAddress: {address}\n");
    }
    text + "\nCould someone check and add it?"
}

/// What a refresh from OpenStreetMap found.
#[derive(Serialize, ToSchema)]
struct RefreshResult {
    status: store::Refreshed,
    /// The place as it is now; `null` when it is gone.
    #[schema(required = true)]
    place: Option<PlaceView>,
}

/// Reads one place again from OpenStreetMap and applies it straight away, instead of at the
/// next daily import: for an edit someone just made there. A place that is gone (deleted,
/// closed, or no longer serving a national cuisine) is hidden from every list. At most 5 a
/// minute.
#[utoipa::path(
    post, path = "/api/places/{place_id}/refresh", tag = "game",
    params(("place_id" = String, Path, description = "An OpenStreetMap place id, like `osm-node-123`."), PlaceQuery),
    security(("token" = [])),
    responses((status = 200, body = RefreshResult), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn refresh_place(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
    Query(query): Query<PlaceQuery>,
) -> ApiResult<RefreshResult> {
    if osm_link(&place_id).is_none() {
        return Err(ApiError::BadRequest(
            "only OpenStreetMap places can be refreshed".into(),
        ));
    }
    store::place_summary(&state.db, &place_id, &user)
        .await?
        .ok_or(ApiError::NotFound("place"))?;
    let now = state.now();
    if !state.limits.allow(&user, "refresh_place", 5, 60, now) {
        return Err(ApiError::TooManyRequests(
            "you refreshed 5 places this minute; try again in a moment",
        ));
    }
    let lookup = state
        .places
        .place(&place_id)
        .await
        .map_err(ApiError::Upstream)?;
    let status = store::refresh_place(&state.db, &place_id, &lookup, now).await?;
    let place = match lookup {
        Lookup::Gone => None,
        Lookup::Found(_) => {
            store::place(&state.db, &place_id, Some(&user))
                .await?
                .map(|mut place| {
                    let (from_lat, from_lng) = match (query.lat, query.lng) {
                        (Some(lat), Some(lng)) => (lat, lng),
                        _ => (place.lat, place.lng),
                    };
                    complete(&mut place, from_lat, from_lng, now);
                    if let Some(h) = &mut place.hours {
                        h.week = hours::week(&h.raw, place.lat, place.lng, now);
                    }
                    place
                })
        }
    };
    tracing::info!(place_id, ?status, "place refreshed from osm");
    Ok(Json(RefreshResult { status, place }))
}

/// How far around the user a nearby refresh looks.
const NEARBY_RADIUS_M: f64 = 150.0;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct NearbyQuery {
    /// Where the user is.
    lat: f64,
    lng: f64,
}

/// What a nearby refresh found.
#[derive(Serialize, ToSchema)]
struct NearbyResult {
    /// Places that weren't listed before, nearest first.
    added: Vec<PlaceView>,
    /// Pending additions, anyone's, now matched to one of the places found.
    matched: u32,
}

/// Asks OpenStreetMap for restaurants within 150 m and adds the new ones straight away,
/// instead of at the next daily import: for a place someone just mapped there. Pending
/// additions that match one are confirmed as by the import. At most 5 an hour.
#[utoipa::path(
    post, path = "/api/places/nearby/refresh", tag = "game", params(NearbyQuery),
    security(("token" = [])),
    responses((status = 200, body = NearbyResult), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn refresh_nearby(
    State(state): State<AppState>,
    User(user): User,
    Query(query): Query<NearbyQuery>,
) -> ApiResult<NearbyResult> {
    let (lat, lng) = (query.lat, query.lng);
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lng) {
        return Err(ApiError::BadRequest("lat/lng out of range".into()));
    }
    let now = state.now();
    if !state.limits.allow(&user, "refresh_nearby", 5, 3600, now) {
        return Err(ApiError::TooManyRequests(
            "you looked again 5 times this hour; try again later",
        ));
    }
    let found = state
        .places
        .nearby(lat, lng, NEARBY_RADIUS_M)
        .await
        .map_err(ApiError::Upstream)?;
    let (new, matched) = store::add_nearby(&state.db, &found, now).await?;
    let mut added = Vec::new();
    for id in new {
        if let Some(mut place) = store::place(&state.db, &id, Some(&user)).await? {
            complete(&mut place, lat, lng, now);
            added.push(place);
        }
    }
    added.sort_by_key(|p| p.distance_m);
    tracing::info!(
        found = found.len(),
        added = added.len(),
        matched,
        "nearby places refreshed from osm"
    );
    Ok(Json(NearbyResult {
        added,
        matched: matched as u32,
    }))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct PlaceQuery {
    /// Where the user is, for the distance. Without it the distance is 0.
    lat: Option<f64>,
    lng: Option<f64>,
}

/// One place with all of the user's visits, newest first. Works for any place id, whatever
/// area it is in, so links such as `?place=osm-node-123` can open it directly.
#[utoipa::path(
    get, path = "/api/places/{place_id}", tag = "game",
    params(("place_id" = String, Path, description = "A place id from `/api/places`."), PlaceQuery),
    security((), ("token" = [])),
    responses((status = 200, body = PlaceView), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn place(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Path(place_id): Path<String>,
    Query(query): Query<PlaceQuery>,
) -> ApiResult<PlaceView> {
    let mut place = store::place(&state.db, &place_id, user.as_deref())
        .await?
        .ok_or(ApiError::NotFound("place"))?;
    let (from_lat, from_lng) = match (query.lat, query.lng) {
        (Some(lat), Some(lng)) => (lat, lng),
        _ => (place.lat, place.lng),
    };
    let now = state.now();
    complete(&mut place, from_lat, from_lng, now);
    if let Some(h) = &mut place.hours {
        h.week = hours::week(&h.raw, place.lat, place.lng, now);
    }
    Ok(Json(place))
}

#[derive(Deserialize, ToSchema)]
struct LatLng {
    lat: f64,
    lng: f64,
}

#[derive(Deserialize, Default, ToSchema)]
struct CheckIn {
    /// Where the user is right now. Optional; used to mark the visit as verified.
    position: Option<LatLng>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct VisitQuery {
    /// As for `/api/discover`: the check-in counts for events by its date in this zone.
    tz: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct VisitResult {
    visit: Visit,
    profile: game::Profile,
}

/// Checks in at a place. There is at most one visit per place per day: checking in again the
/// same day returns that day's visit, verified if either check-in was made on the spot.
#[utoipa::path(
    post, path = "/api/places/{place_id}/visits", tag = "game",
    params(("place_id" = String, Path, description = "A place id from `/api/places`."), VisitQuery),
    request_body(
        content = Option<CheckIn>,
        description = "Optional. Sent with `Content-Type: application/json`, or it is ignored.",
    ),
    security(("token" = [])),
    responses((status = 200, body = VisitResult), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn add_visit(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
    Query(query): Query<VisitQuery>,
    body: Option<JsonBody<CheckIn>>,
) -> ApiResult<VisitResult> {
    let (_, plat, plng) = store::place_summary(&state.db, &place_id, &user)
        .await?
        .ok_or(ApiError::NotFound("place"))?;
    let check_in = body.map(|JsonBody(b)| b).unwrap_or_default();
    let verified = check_in
        .position
        .is_some_and(|p| geo::distance_m(p.lat, p.lng, plat, plng) <= VERIFY_DISTANCE_M);
    let visit = store::add_visit(
        &state.db,
        &user,
        &place_id,
        verified,
        state.now(),
        state.today(zone(query.tz.as_deref())),
    )
    .await?;
    let profile = game::profile(store::lifetime(&state.db, &user).await?);
    Ok(Json(VisitResult { visit, profile }))
}

/// Removes one visit. The place stays visited while the user has other visits to it.
#[utoipa::path(
    delete, path = "/api/visits/{visit_id}", tag = "game",
    params(("visit_id" = i64, Path, description = "A visit id from a place's `visits`.")),
    security(("token" = [])),
    responses((status = 204, description = "Removed, or there was no such visit"), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn remove_visit(
    State(state): State<AppState>,
    User(user): User,
    Path(visit_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    store::remove_visit(&state.db, &user, visit_id).await?;
    Ok(StatusCode::NO_CONTENT)
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
    let local = host.starts_with("localhost") || host.starts_with("127.0.0.1") || host.starts_with("10.0.2.2");
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .unwrap_or(if local { "http" } else { "https" });
    format!("{proto}://{host}")
}

fn host(headers: &header::HeaderMap) -> String {
    origin(headers).split("://").nth(1).unwrap_or_default().to_string()
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
    let area = resolve_area(&state, &query.area).await?;
    let slug = share::publish_profile(&state.db, &user, area.id(), state.now()).await?;
    let rows = store::area_rows(&state.db, area.id(), Some(&user)).await?;
    let summary = game::summarize(&rows, store::lifetime(&state.db, &user).await?);
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
async fn unshare_profile(State(state): State<AppState>, User(user): User) -> Result<StatusCode, ApiError> {
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
    let visit = share::find_visit(&state.db, &slug).await?.ok_or(ApiError::NotFound("visit"))?;
    let url = format!("{}/c/{slug}", origin(&headers));
    Ok(Json(ShareLink {
        image_url: format!("{url}/card.png"),
        url,
        text: format!("{} on Mangiamundo.", share::visit_headline(&visit)),
    }))
}

/// A public page's progress: the summary and the area's name.
async fn public_profile(state: &AppState, slug: &str) -> Result<(Summary, String), ApiError> {
    let (user, area_id) = share::find_profile(&state.db, slug).await?.ok_or(ApiError::NotFound("page"))?;
    let area = store::find_area(&state.db, &area_id).await?.ok_or(ApiError::NotFound("page"))?;
    let rows = store::area_rows(&state.db, &area_id, Some(&user)).await?;
    let summary = game::summarize(&rows, store::lifetime(&state.db, &user).await?);
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
        [(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "no-store")],
        bytes,
    )
        .into_response()
}

/// Draws a card off the async threads; it takes a few dozen milliseconds.
async fn render_card(statuses: HashMap<String, game::Status>, headline: String, caption: String) -> Result<Vec<u8>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let statuses = statuses.iter().map(|(c, s)| (c.as_str(), *s)).collect();
        share::card_png(&statuses, &headline, &caption)
    })
    .await
    .map_err(|e| ApiError::Internal(e.into()))?
    .map_err(ApiError::Internal)
}

async fn profile_page(State(state): State<AppState>, headers: header::HeaderMap, Path(slug): Path<String>) -> Result<Response, ApiError> {
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

async fn profile_card(State(state): State<AppState>, headers: header::HeaderMap, Path(slug): Path<String>) -> Result<Response, ApiError> {
    let (summary, area) = public_profile(&state, &slug).await?;
    let statuses = summary.countries.iter().map(|c| (c.code.clone(), c.status)).collect();
    let caption = format!("{area} · Level {} · {}", summary.profile.level, host(&headers));
    Ok(png(render_card(statuses, share::profile_headline(&summary), caption).await?))
}

async fn visit_page(State(state): State<AppState>, headers: header::HeaderMap, Path(slug): Path<String>) -> Result<Response, ApiError> {
    let visit = share::find_visit(&state.db, &slug).await?.ok_or(ApiError::NotFound("page"))?;
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

async fn visit_card(State(state): State<AppState>, headers: header::HeaderMap, Path(slug): Path<String>) -> Result<Response, ApiError> {
    let visit = share::find_visit(&state.db, &slug).await?.ok_or(ApiError::NotFound("page"))?;
    // The check-in's countries light up gold, as freshly tasted.
    let statuses = visit.countries.iter().map(|c| (c.clone(), game::Status::Discovered)).collect();
    let caption = format!("{} · {}", share::pretty_date(&visit.date), host(&headers));
    Ok(png(render_card(statuses, share::visit_headline(&visit), caption).await?))
}

/// "This place has closed": hides it for the reporter now, and for everyone after more reports.
#[utoipa::path(
    put, path = "/api/places/{place_id}/closed", tag = "game", params(("place_id" = String, Path, description = "A place id from `/api/places`.")),
    security(("token" = [])),
    responses((status = 204, description = "Reported"), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn report_closed(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let (name, lat, lng) = store::place_summary(&state.db, &place_id, &user)
        .await?
        .ok_or(ApiError::NotFound("place"))?;
    let reports = store::report_closed(&state.db, &user, &place_id, state.now()).await?;
    // Tell OSM once, when the place gets hidden for everyone, so one person cannot spam mappers.
    if let (Some(2), Some(url), Some(osm)) = (reports, &state.osm_notes_url, osm_link(&place_id)) {
        let text = format!(
            "Two Mangiamundo users reported that \"{name}\" ({osm}) has closed. \
             Could someone check and update or remove it?"
        );
        tokio::spawn(post_osm_note(
            url.clone(),
            state.osm_notes_token.clone(),
            lat,
            lng,
            text,
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// The openstreetmap.org page of an OSM place, from an id like `osm-node-123`.
fn osm_link(place_id: &str) -> Option<String> {
    let (kind, id) = place_id.strip_prefix("osm-")?.split_once('-')?;
    Some(format!("https://www.openstreetmap.org/{kind}/{id}"))
}

/// Posts an OSM note, as the account `token` belongs to or anonymously, and returns its id.
/// Best effort: a failure is logged, not retried.
async fn post_osm_note(
    url: String,
    token: Option<String>,
    lat: f64,
    lng: f64,
    text: String,
) -> Option<i64> {
    // The `.json` endpoint answers with the note as GeoJSON instead of XML.
    let result = async {
        let mut request = reqwest::Client::new()
            .post(format!("{url}.json"))
            .header(
                header::USER_AGENT,
                concat!("mangiamundo/", env!("CARGO_PKG_VERSION")),
            )
            .form(&[
                ("lat", lat.to_string()),
                ("lon", lng.to_string()),
                ("text", text),
            ]);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        request
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await
    }
    .await;
    match result {
        Ok(note) => {
            let id = note["properties"]["id"].as_i64();
            tracing::info!(lat, lng, id, "posted osm note");
            id
        }
        Err(error) => {
            tracing::warn!(%error, "osm note failed");
            None
        }
    }
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

/// A browser's PushSubscription (as from `subscription.toJSON()`), plus the user's area.
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
    let subscription = crate::push::Subscription {
        endpoint: body.endpoint,
        p256dh: body.keys.p256dh,
        auth: body.keys.auth,
        area_id: Some(resolve_area(&state, &body.area).await?.id().to_string()),
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
#[utoipa::path(
    get, path = "/api/areas", tag = "areas", params(SearchQuery),
    responses((status = 200, body = Vec<AreaInfo>), (status = "default", description = "An error", body = ErrorBody)),
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
        .map_err(ApiError::Upstream)
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
    responses((status = 200, body = Located), (status = "default", description = "An error", body = ErrorBody)),
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
        .map_err(ApiError::Upstream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_estimate_slows_down_and_never_reaches_the_end_of_its_step() {
        assert_eq!(estimate(15.0, 95.0, 0.0, 20.0), 15.0);
        let after_expected = estimate(15.0, 95.0, 20.0, 20.0);
        assert!((65.0..66.0).contains(&after_expected), "{after_expected}");
        assert!(estimate(15.0, 95.0, 600.0, 20.0) < 95.0);
    }

    #[test]
    fn progress_stays_until_the_last_concurrent_fetch_ends() {
        let loads = Loads::default();
        let first = loads.begin("leuven");
        let second = loads.begin("leuven");
        loads.step("leuven", 15.0, 95.0, "Looking for restaurants", 20.0);
        assert_eq!(
            loads.progress("leuven"),
            Some((15, "Looking for restaurants"))
        );
        drop(first);
        assert_eq!(
            loads.progress("leuven"),
            Some((15, "Looking for restaurants"))
        );
        drop(second);
        assert_eq!(loads.progress("leuven"), None);
    }
}
