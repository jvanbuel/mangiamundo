//! HTTP API. Everything lives under `/api` and speaks JSON, so the web app and the
//! mobile apps share one contract.

use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use std::collections::HashMap;

use chrono::{Duration, NaiveDate};

use crate::calendar::{self, Celebration};
use crate::game::{self, Lifetime, PlaceRow, Summary};
use crate::providers::{AreaInfo, Located};
use crate::store::{self, Area, PlaceView};
use crate::{catalog, geo, hours, AppState};

/// A check-in counts as verified when made within this distance of the place.
const VERIFY_DISTANCE_M: f64 = 250.0;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(|| async { Json(json!({ "ok": true })) }))
        .route("/api/session", post(create_session))
        .route("/api/countries", get(countries))
        .route("/api/discover", get(discover))
        .route("/api/places", get(places))
        .route("/api/celebrations", get(celebrations))
        .route("/api/places/{place_id}/closed", put(report_closed))
        .route("/api/visits/{place_id}", put(add_visit).delete(remove_visit))
        .route("/api/push/key", get(push_key))
        .route("/api/push/subscription", put(subscribe).delete(unsubscribe))
        .route("/api/areas", get(search_areas))
        .route("/api/areas/locate", get(locate_area))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
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
        let status = match &self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Upstream(error) => {
                tracing::error!(error = format!("{error:#}"), "provider request failed");
                StatusCode::BAD_GATEWAY
            }
            ApiError::Database(error) => {
                tracing::error!(%error, "database error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
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

/// Accounts are anonymous: a device asks for a token once and keeps it.
async fn create_session(State(state): State<AppState>) -> ApiResult<serde_json::Value> {
    let token = store::create_user(&state.db).await?;
    Ok(Json(json!({ "token": token })))
}

#[derive(Serialize)]
struct Catalog {
    countries: &'static [catalog::Country],
    continents: Vec<serde_json::Value>,
}

async fn countries() -> Json<Catalog> {
    Json(Catalog {
        countries: catalog::countries(),
        continents: catalog::CONTINENTS
            .iter()
            .map(|(code, name)| json!({ "code": code, "name": name }))
            .collect(),
    })
}

/// The largest area served, in degrees of latitude and longitude: a big city fits, a
/// region or a country (which would make for a huge Overpass query) doesn't.
const MAX_AREA_DEGREES: f64 = 1.5;

#[derive(Deserialize)]
struct AreaQuery {
    /// An area id from `/api/areas` or `/api/areas/locate`.
    area: String,
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
#[derive(Serialize)]
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

#[derive(Serialize)]
struct Discover {
    area: AreaView,
    #[serde(flatten)]
    summary: Summary,
    events: Vec<EventProgress>,
}

#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
enum EventStatus {
    Upcoming,
    Active,
    Ended,
}

/// A time-limited challenge and how far the user got.
#[derive(Serialize)]
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

/// Unix time at the start of a UTC day.
fn day_start(day: NaiveDate) -> i64 {
    day.and_hms_opt(0, 0, 0).map(|t| t.and_utc().timestamp()).unwrap_or(0)
}

/// Events to show: upcoming within a month, running, or finished and earned.
async fn event_progress(
    state: &AppState,
    user: Option<&str>,
    rows: &[PlaceRow],
) -> Result<Vec<EventProgress>, ApiError> {
    let today = state.today();
    let counts = places_per_country(rows);
    let mut out = Vec::new();
    for event in calendar::relevant_events(today, 30) {
        let progress = match user {
            Some(user) => {
                let until = day_start(event.ends + Duration::days(1));
                store::visits_between(&state.db, user, &event.countries, day_start(event.starts), until).await?
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

async fn discover(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Query(query): Query<AreaQuery>,
) -> ApiResult<Discover> {
    let area = resolve_area(&state, &query.area).await?;
    let rows = store::area_rows(&state.db, area.id(), user.as_deref()).await?;
    let lifetime = match &user {
        Some(user) => store::lifetime(&state.db, user).await?,
        None => Lifetime::default(),
    };
    let events = event_progress(&state, user.as_deref(), &rows).await?;
    Ok(Json(Discover { summary: game::summarize(&rows, lifetime), area: (&area).into(), events }))
}

#[derive(Deserialize)]
struct CelebrationsQuery {
    area: String,
    /// How far ahead to look.
    days: Option<i64>,
}

#[derive(Serialize)]
struct NearbyCountry {
    code: String,
    places: u32,
}

#[derive(Serialize)]
struct CelebrationView {
    #[serde(flatten)]
    celebration: Celebration,
    /// The celebration's countries that have places in the area.
    nearby: Vec<NearbyCountry>,
    /// Notification text for the first day, for apps that schedule reminders themselves.
    reminder: crate::push::Notification,
}

/// Celebrations coming up for countries whose food can be found in the area.
async fn celebrations(
    State(state): State<AppState>,
    Query(query): Query<CelebrationsQuery>,
) -> ApiResult<Vec<CelebrationView>> {
    let area = resolve_area(&state, &query.area).await?;
    let rows = store::area_rows(&state.db, area.id(), None).await?;
    let counts = places_per_country(&rows);
    let days = query.days.unwrap_or(60).clamp(0, 366);
    Ok(Json(nearby_celebrations(state.today(), days, &counts)))
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

#[derive(Deserialize)]
struct PlacesQuery {
    area: String,
    country: Option<String>,
    /// Where the user is, for distances. Without it they are measured from the area's centre.
    lat: Option<f64>,
    lng: Option<f64>,
}

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
        place.distance_m = geo::distance_m(from_lat, from_lng, place.lat, place.lng).round() as u32;
        if let Some((open, text)) =
            place.opening_hours.as_deref().and_then(|raw| hours::status(raw, place.lat, place.lng, now))
        {
            place.open_now = Some(open);
            place.hours_text = Some(text);
        }
    }
    // Open first, then closed, then unknown hours; alphabetical within each group.
    places.sort_by_cached_key(|p| (group(p.open_now), p.name.to_lowercase()));
    Ok(Json(places))
}

fn group(open_now: Option<bool>) -> u8 {
    match open_now {
        Some(true) => 0,
        Some(false) => 1,
        None => 2,
    }
}

#[derive(Deserialize, Default)]
struct CheckIn {
    /// Where the user is right now. Optional; used to mark the visit as verified.
    lat: Option<f64>,
    lng: Option<f64>,
}

#[derive(Serialize)]
struct VisitResult {
    place_id: String,
    verified: bool,
    profile: game::Profile,
}

async fn add_visit(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
    body: Option<Json<CheckIn>>,
) -> ApiResult<VisitResult> {
    let (plat, plng) = store::place_location(&state.db, &place_id)
        .await?
        .ok_or(ApiError::NotFound("place"))?;
    let check_in = body.map(|Json(b)| b).unwrap_or_default();
    let verified = match (check_in.lat, check_in.lng) {
        (Some(lat), Some(lng)) => geo::distance_m(lat, lng, plat, plng) <= VERIFY_DISTANCE_M,
        _ => false,
    };
    store::add_visit(&state.db, &user, &place_id, verified, state.now()).await?;
    let profile = game::profile(store::lifetime(&state.db, &user).await?);
    Ok(Json(VisitResult { place_id, verified, profile }))
}

async fn remove_visit(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    store::remove_visit(&state.db, &user, &place_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// "This place has closed": hides it for the reporter now, and for everyone after more reports.
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

async fn push_key(State(state): State<AppState>) -> ApiResult<serde_json::Value> {
    let key = state.push_key.clone().ok_or(ApiError::NotFound("push"))?;
    Ok(Json(json!({ "public_key": key })))
}

/// A browser's PushSubscription (as from `subscription.toJSON()`), plus the user's area.
#[derive(Deserialize)]
struct SubscribeBody {
    endpoint: String,
    keys: SubscribeKeys,
    area: String,
    timezone: String,
}

#[derive(Deserialize)]
struct SubscribeKeys {
    p256dh: String,
    auth: String,
}

async fn subscribe(
    State(state): State<AppState>,
    User(user): User,
    Json(body): Json<SubscribeBody>,
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

#[derive(Deserialize)]
struct UnsubscribeBody {
    endpoint: String,
}

async fn unsubscribe(
    State(state): State<AppState>,
    User(user): User,
    Json(body): Json<UnsubscribeBody>,
) -> Result<StatusCode, ApiError> {
    store::remove_user_push_subscription(&state.db, &user, &body.endpoint).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

/// Cities and postal codes matching what someone typed.
async fn search_areas(State(state): State<AppState>, Query(query): Query<SearchQuery>) -> ApiResult<Vec<AreaInfo>> {
    let q = query.q.trim();
    if q.chars().count() < 2 {
        return Ok(Json(Vec::new()));
    }
    state.areas.search(q).await.map(Json).map_err(ApiError::Upstream)
}

#[derive(Deserialize)]
struct LocateQuery {
    lat: f64,
    lng: f64,
}

/// The city and postal code a point (usually the device's position) lies in.
async fn locate_area(State(state): State<AppState>, Query(query): Query<LocateQuery>) -> ApiResult<Located> {
    if !(-90.0..=90.0).contains(&query.lat) || !(-180.0..=180.0).contains(&query.lng) {
        return Err(ApiError::BadRequest("lat/lng out of range".into()));
    }
    state.areas.locate(query.lat, query.lng).await.map(Json).map_err(ApiError::Upstream)
}
