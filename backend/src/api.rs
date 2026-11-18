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

use chrono::NaiveDate;
use chrono_tz::Tz;

use crate::calendar::{self, Celebration};
use crate::game::{self, Lifetime, PlaceRow, Summary};
use crate::providers::{AreaInfo, Located};
use crate::store::{self, Area, HoursStatus, PlaceView, Visit};
use crate::{catalog, geo, hours, AppState};

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
        components.add_security_scheme("token", SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()));
    }
}

/// The documented API routes and the OpenAPI document that describes them. A route
/// registered with `routes!` can't exist without being in the document.
fn api_router() -> (Router<AppState>, utoipa::openapi::OpenApi) {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health))
        .routes(routes!(create_session))
        .routes(routes!(countries))
        .routes(routes!(discover))
        .routes(routes!(places))
        .routes(routes!(place))
        .routes(routes!(celebrations))
        .routes(routes!(report_closed))
        .routes(routes!(add_visit))
        .routes(routes!(remove_visit))
        .routes(routes!(push_key))
        .routes(routes!(subscribe))
        .routes(routes!(unsubscribe))
        .routes(routes!(search_areas))
        .routes(routes!(locate_area))
        .routes(routes!(suggest_areas))
        .split_for_parts()
}

pub fn router(state: AppState) -> Router {
    let (api, doc) = api_router();
    // The document itself and the catch-alls below are the only routes not in it.
    api.route("/api/openapi.json", get(move || std::future::ready(Json(doc.clone()))))
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
    Json(Health { ok: true, version: env!("CARGO_PKG_VERSION"), commit: option_env!("GIT_COMMIT") })
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
/// - `not_found` (404): an unknown place, area or `/api` path.
/// - `method_not_allowed` (405): the path exists, but not with this method.
/// - `upstream` (502): the places or areas provider failed.
/// - `internal` (500): anything else, such as a database error.
#[derive(Serialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    BadRequest,
    Unauthorized,
    NotFound,
    MethodNotAllowed,
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
    #[error("the places provider is unavailable")]
    Upstream(#[source] anyhow::Error),
    #[error("internal error")]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, ErrorCode::BadRequest),
            ApiError::Rejected(status, _) => (*status, ErrorCode::BadRequest),
            ApiError::MethodNotAllowed => (StatusCode::METHOD_NOT_ALLOWED, ErrorCode::MethodNotAllowed),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, ErrorCode::Unauthorized),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, ErrorCode::NotFound),
            ApiError::Upstream(error) => {
                tracing::error!(error = format!("{error:#}"), "provider request failed");
                (StatusCode::BAD_GATEWAY, ErrorCode::Upstream)
            }
            ApiError::Database(error) => {
                tracing::error!(%error, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::Internal)
            }
        };
        (status, Json(ErrorBody { error: self.to_string(), code })).into_response()
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
        let axum::extract::Query(value) = axum::extract::Query::from_request_parts(parts, state).await?;
        Ok(Query(value))
    }
}

/// axum's `Path`, but a path segment that does not decode is a JSON error like any other.
struct Path<T>(T);

impl<S: Send + Sync, T: DeserializeOwned + Send> FromRequestParts<S> for Path<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        let axum::extract::Path(value) = axum::extract::Path::from_request_parts(parts, state).await?;
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

/// The user behind the `Authorization: Bearer <token>` header, if there is one.
struct MaybeUser(Option<String>);
/// Like [`MaybeUser`] but rejects anonymous requests.
struct User(String);

impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let Some(value) = parts.headers.get(header::AUTHORIZATION) else {
            return Ok(MaybeUser(None));
        };
        let token = value
            .to_str()
            .ok()
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        match store::user_for_token(&state.db, token).await? {
            Some(user) => Ok(MaybeUser(Some(user))),
            None => Err(ApiError::Unauthorized),
        }
    }
}

impl FromRequestParts<AppState> for User {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        match MaybeUser::from_request_parts(parts, state).await? {
            MaybeUser(Some(user)) => Ok(User(user)),
            MaybeUser(None) => Err(ApiError::Unauthorized),
        }
    }
}

#[derive(Serialize, ToSchema)]
struct Session {
    /// Send it as `Authorization: Bearer <token>`.
    token: String,
}

/// Creates an anonymous account. A device asks for a token once and keeps it.
#[utoipa::path(post, path = "/api/session", tag = "account", responses((status = 200, body = Session), (status = "default", description = "An error", body = ErrorBody)))]
async fn create_session(State(state): State<AppState>) -> ApiResult<Session> {
    let token = store::create_user(&state.db).await?;
    Ok(Json(Session { token }))
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
        continents: catalog::CONTINENTS.iter().map(|&(code, name)| Continent { code, name }).collect(),
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
    !id.is_empty() && id.len() <= 100 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
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

    let boundary = match &cached {
        // The outline rarely changes; keep it for refreshes.
        Some(area) => area.boundary.clone(),
        None => {
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
    match state.places.search(&boundary).await {
        Ok(found) => {
            let area = Area { boundary, provider: state.places.name().into(), fetched_at: state.now() };
            store::save_area(&state.db, &area, &found).await?;
            tracing::info!(area = area.id(), places = found.len(), "area refreshed");
            Ok(area)
        }
        Err(error) => match cached {
            Some(area) => {
                tracing::warn!(error = format!("{error:#}"), "provider failed, serving stale area");
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
        AreaView { info: area.boundary.info.clone(), fetched_at: area.fetched_at }
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
                store::visits_between(&state.db, user, &event.countries, event.starts, event.ends).await?
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
            places_nearby: event.countries.iter().map(|c| counts.get(c.as_str()).copied().unwrap_or(0)).sum(),
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
    Ok(Json(Discover { summary: game::summarize(&rows, lifetime), area: (&area).into(), events }))
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

fn nearby_celebrations(today: NaiveDate, days: i64, counts: &HashMap<&str, u32>) -> Vec<CelebrationView> {
    calendar::upcoming(today, days)
        .into_iter()
        .filter_map(|celebration| {
            let nearby: Vec<NearbyCountry> = celebration
                .countries
                .iter()
                .filter_map(|code| Some(NearbyCountry { code: code.clone(), places: *counts.get(code.as_str())? }))
                .collect();
            let owned: HashMap<String, u32> = nearby.iter().map(|n| (n.code.clone(), n.places)).collect();
            let reminder = crate::push::celebration_notification(&celebration, &owned)?;
            Some(CelebrationView { celebration, nearby, reminder })
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
    let mut places = store::area_places(&state.db, area.id(), country.as_deref(), user.as_deref()).await?;
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

/// Fills in what the store leaves empty: the distance and whether the place is open now.
fn complete(place: &mut PlaceView, from_lat: f64, from_lng: f64, now: i64) {
    place.distance_m = geo::distance_m(from_lat, from_lng, place.lat, place.lng).round() as u32;
    if let Some(h) = &mut place.hours {
        h.status = hours::status(&h.raw, place.lat, place.lng, now).map(|(open, text)| HoursStatus { open, text });
    }
}

fn group(place: &PlaceView) -> u8 {
    match place.hours.as_ref().and_then(|h| h.status.as_ref()) {
        Some(HoursStatus { open: true, .. }) => 0,
        Some(HoursStatus { open: false, .. }) => 1,
        None => 2,
    }
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
    let mut place = store::place(&state.db, &place_id, user.as_deref()).await?.ok_or(ApiError::NotFound("place"))?;
    let (from_lat, from_lng) = match (query.lat, query.lng) {
        (Some(lat), Some(lng)) => (lat, lng),
        _ => (place.lat, place.lng),
    };
    complete(&mut place, from_lat, from_lng, state.now());
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
    let (plat, plng) = store::place_location(&state.db, &place_id)
        .await?
        .ok_or(ApiError::NotFound("place"))?;
    let check_in = body.map(|JsonBody(b)| b).unwrap_or_default();
    let verified = check_in
        .position
        .is_some_and(|p| geo::distance_m(p.lat, p.lng, plat, plng) <= VERIFY_DISTANCE_M);
    let visit = store::add_visit(&state.db, &user, &place_id, verified, state.now(), state.today(zone(query.tz.as_deref())))
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
    let (name, lat, lng) = store::place_summary(&state.db, &place_id).await?.ok_or(ApiError::NotFound("place"))?;
    let reports = store::report_closed(&state.db, &user, &place_id, state.now()).await?;
    // Tell OSM once, when the place gets hidden for everyone, so one person cannot spam mappers.
    if let (Some(2), Some(url), Some(osm)) = (reports, &state.osm_notes_url, osm_link(&place_id)) {
        let text = format!(
            "Two Mangiamundo users reported that \"{name}\" ({osm}) has closed. \
             Could someone check and update or remove it?"
        );
        tokio::spawn(post_osm_note(url.clone(), lat, lng, text));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// The openstreetmap.org page of an OSM place, from an id like `osm-node-123`.
fn osm_link(place_id: &str) -> Option<String> {
    let (kind, id) = place_id.strip_prefix("osm-")?.split_once('-')?;
    Some(format!("https://www.openstreetmap.org/{kind}/{id}"))
}

/// Posts an anonymous OSM note. Best effort: a failure is logged, not retried.
async fn post_osm_note(url: String, lat: f64, lng: f64, text: String) {
    let result = reqwest::Client::new()
        .post(&url)
        .header(header::USER_AGENT, concat!("mangiamundo/", env!("CARGO_PKG_VERSION")))
        .form(&[("lat", lat.to_string()), ("lon", lng.to_string()), ("text", text)])
        .send()
        .await
        .and_then(|r| r.error_for_status());
    match result {
        Ok(_) => tracing::info!(lat, lng, "posted osm note"),
        Err(error) => tracing::warn!(%error, "osm note failed"),
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
async fn search_areas(State(state): State<AppState>, Query(query): Query<SearchQuery>) -> ApiResult<Vec<AreaInfo>> {
    let q = query.q.trim();
    if q.chars().count() < 2 {
        return Ok(Json(Vec::new()));
    }
    state.areas.search(q).await.map(Json).map_err(ApiError::Upstream)
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
async fn suggest_areas(State(state): State<AppState>, Query(query): Query<SuggestQuery>) -> ApiResult<Vec<AreaInfo>> {
    let q = query.q.trim();
    if q.chars().count() < 2 {
        return Ok(Json(Vec::new()));
    }
    let near = query.lat.zip(query.lng);
    state.areas.suggest(q, near).await.map(Json).map_err(ApiError::Upstream)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct LocateQuery {
    lat: f64,
    lng: f64,
}

/// The city and postal code a point (usually the device's position) lies in.
#[utoipa::path(
    get, path = "/api/areas/locate", tag = "areas", params(LocateQuery),
    responses((status = 200, body = Located), (status = "default", description = "An error", body = ErrorBody)),
)]
async fn locate_area(State(state): State<AppState>, Query(query): Query<LocateQuery>) -> ApiResult<Located> {
    if !(-90.0..=90.0).contains(&query.lat) || !(-180.0..=180.0).contains(&query.lng) {
        return Err(ApiError::BadRequest("lat/lng out of range".into()));
    }
    state.areas.locate(query.lat, query.lng).await.map(Json).map_err(ApiError::Upstream)
}
