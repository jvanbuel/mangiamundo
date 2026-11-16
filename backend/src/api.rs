//! HTTP API. Everything lives under `/api` and speaks JSON, so the web app and the
//! mobile apps share one contract.

use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use std::collections::HashMap;

use chrono::{Duration, NaiveDate};

use crate::calendar::{self, Celebration};
use crate::game::{self, Lifetime, PlaceRow, Summary};
use crate::providers::GeoResult;
use crate::store::{self, Area, PlaceView};
use crate::{catalog, geo, AppState};

/// A check-in counts as verified when made within this distance of the place.
const VERIFY_DISTANCE_M: f64 = 250.0;
/// Time between two check-ins at the same place, so one meal counts once.
pub const CHECKIN_COOLDOWN_SECS: i64 = 12 * 3600;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(|| async { Json(json!({ "ok": true })) }))
        .route("/api/session", post(create_session))
        .route("/api/session/recovery", get(recovery_code).post(rotate_recovery_code))
        .route("/api/session/restore", post(restore_session))
        .route("/api/countries", get(countries))
        .route("/api/discover", get(discover))
        .route("/api/places", get(places))
        .route("/api/celebrations", get(celebrations))
        .route("/api/visits/{place_id}", put(add_visit).post(check_in).delete(remove_visits))
        .route("/api/checkins/{id}", patch(update_checkin).delete(remove_checkin))
        .route("/api/share/card.png", get(crate::share::card))
        .route("/api/push/key", get(push_key))
        .route("/api/push/subscription", put(subscribe).delete(unsubscribe))
        .route("/api/geocode", get(geocode))
        .route("/api/geocode/reverse", get(reverse_geocode))
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
    #[error("{0}")]
    Conflict(String),
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
            ApiError::Conflict(_) => StatusCode::CONFLICT,
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

pub(crate) type ApiResult<T> = Result<Json<T>, ApiError>;

/// The user behind the `Authorization: Bearer <token>` header, if there is one.
pub(crate) struct MaybeUser(pub Option<String>);
/// Like [`MaybeUser`] but rejects anonymous requests.
pub(crate) struct User(pub String);

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
struct RecoveryCode {
    /// Formatted in groups of four for reading and writing down.
    code: String,
}

fn recovery_view(code: &str) -> Json<RecoveryCode> {
    let groups: Vec<&str> = code.as_bytes().chunks(4).map(|c| std::str::from_utf8(c).unwrap_or("")).collect();
    Json(RecoveryCode { code: groups.join("-") })
}

/// The code that restores this passport on another device.
async fn recovery_code(State(state): State<AppState>, User(user): User) -> ApiResult<RecoveryCode> {
    Ok(recovery_view(&store::recovery_code(&state.db, &user).await?))
}

/// Replaces the recovery code; the old one stops working.
async fn rotate_recovery_code(State(state): State<AppState>, User(user): User) -> ApiResult<RecoveryCode> {
    Ok(recovery_view(&store::rotate_recovery_code(&state.db, &user).await?))
}

#[derive(Deserialize)]
struct RestoreBody {
    code: String,
}

#[derive(Serialize)]
struct Restored {
    token: String,
    /// Check-ins this device had made on its own account and brought along.
    merged: u64,
}

/// Swaps this device onto the passport behind a recovery code. Whatever the device had
/// collected under its own account is merged in, so nothing is lost either way.
async fn restore_session(
    State(state): State<AppState>,
    MaybeUser(current): MaybeUser,
    Json(body): Json<RestoreBody>,
) -> ApiResult<Restored> {
    let code = store::normalize_recovery_code(&body.code)
        .ok_or_else(|| ApiError::BadRequest("a recovery code has 16 letters and digits".into()))?;
    let (target, token) = store::user_for_recovery_code(&state.db, &code)
        .await?
        .ok_or(ApiError::NotFound("recovery code"))?;
    let merged = match current {
        Some(current) if current != target => store::merge_users(&state.db, &current, &target).await?,
        _ => 0,
    };
    Ok(Json(Restored { token, merged }))
}

#[derive(Serialize)]
struct Catalog {
    countries: &'static [catalog::Country],
    continents: Vec<serde_json::Value>,
    dishes: &'static [catalog::Dish],
}

async fn countries() -> Json<Catalog> {
    Json(Catalog {
        dishes: catalog::dishes(),
        countries: catalog::countries(),
        continents: catalog::CONTINENTS
            .iter()
            .map(|(code, name)| json!({ "code": code, "name": name }))
            .collect(),
    })
}

#[derive(Deserialize)]
pub(crate) struct AreaQuery {
    pub lat: f64,
    pub lng: f64,
    pub radius_km: Option<f64>,
}

impl AreaQuery {
    fn validate(&self) -> Result<u32, ApiError> {
        if !(-90.0..=90.0).contains(&self.lat) || !(-180.0..=180.0).contains(&self.lng) {
            return Err(ApiError::BadRequest("lat/lng out of range".into()));
        }
        let radius_km = self.radius_km.unwrap_or(10.0);
        if !(1.0..=50.0).contains(&radius_km) {
            return Err(ApiError::BadRequest("radius_km must be between 1 and 50".into()));
        }
        Ok((radius_km * 1000.0).round() as u32)
    }
}

/// Returns the cached area for a location, asking the provider when the cache is
/// missing or stale. A stale cache is still used when the provider is down.
pub(crate) async fn resolve_area(state: &AppState, query: &AreaQuery) -> Result<Area, ApiError> {
    let radius_m = query.validate()?;
    let (lat, lng) = store::snap(query.lat, query.lng);
    let provider = state.places.name();
    let key = store::area_key(provider, lat, lng, radius_m);

    let cached = store::find_area(&state.db, &key).await?;
    if let Some(area) = &cached {
        if state.now() - area.fetched_at < state.cache_ttl_secs {
            return Ok(area.clone());
        }
    }

    match state.places.search(lat, lng, radius_m).await {
        Ok(found) => {
            let area = Area { key, lat, lng, radius_m, provider: provider.into(), fetched_at: state.now() };
            store::save_area(&state.db, &area, &found).await?;
            tracing::info!(key = area.key, places = found.len(), "area refreshed");
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

#[derive(Serialize)]
struct Discover {
    area: Area,
    #[serde(flatten)]
    summary: Summary,
    events: Vec<EventProgress>,
    /// Ids of the signature dishes the user ever logged.
    dishes_tried: Vec<String>,
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
    let area = resolve_area(&state, &query).await?;
    let rows = store::area_rows(&state.db, &area.key, user.as_deref()).await?;
    let lifetime = match &user {
        Some(user) => store::lifetime(&state.db, user).await?,
        None => Lifetime::default(),
    };
    let events = event_progress(&state, user.as_deref(), &rows).await?;
    let dishes_tried = match &user {
        Some(user) => store::dishes_tried(&state.db, user).await?,
        None => Vec::new(),
    };
    Ok(Json(Discover { summary: game::summarize(&rows, lifetime), area, events, dishes_tried }))
}

#[derive(Deserialize)]
struct CelebrationsQuery {
    lat: f64,
    lng: f64,
    radius_km: Option<f64>,
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
    let area_query = AreaQuery { lat: query.lat, lng: query.lng, radius_km: query.radius_km };
    let area = resolve_area(&state, &area_query).await?;
    let rows = store::area_rows(&state.db, &area.key, None).await?;
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
    lat: f64,
    lng: f64,
    radius_km: Option<f64>,
    country: Option<String>,
}

async fn places(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Query(query): Query<PlacesQuery>,
) -> ApiResult<Vec<PlaceView>> {
    let area_query = AreaQuery { lat: query.lat, lng: query.lng, radius_km: query.radius_km };
    let area = resolve_area(&state, &area_query).await?;
    let country = query.country.map(|c| c.to_uppercase());
    let mut places = store::area_places(&state.db, &area.key, country.as_deref(), user.as_deref()).await?;
    let now = state.now();
    for place in &mut places {
        place.distance_m = geo::distance_m(query.lat, query.lng, place.lat, place.lng).round() as u32;
        place.next_check_in_at = place.last_visited_at.map(|at| at + CHECKIN_COOLDOWN_SECS).filter(|&at| at > now);
    }
    places.sort_by_key(|p| p.distance_m);
    Ok(Json(places))
}

#[derive(Deserialize, Default)]
struct CheckIn {
    /// Where the user is right now. Optional; used to mark the visit as verified.
    lat: Option<f64>,
    lng: Option<f64>,
    /// A signature dish of one of the place's countries.
    dish: Option<String>,
}

#[derive(Serialize)]
struct VisitResult {
    place_id: String,
    checkin_id: i64,
    verified: bool,
    profile: game::Profile,
}

/// Whether a check-in made from `body`'s position counts as on the spot.
async fn verify(state: &AppState, place_id: &str, body: &CheckIn) -> Result<bool, ApiError> {
    let (plat, plng) = store::place_location(&state.db, place_id).await?.ok_or(ApiError::NotFound("place"))?;
    Ok(match (body.lat, body.lng) {
        (Some(lat), Some(lng)) => geo::distance_m(lat, lng, plat, plng) <= VERIFY_DISTANCE_M,
        _ => false,
    })
}

/// Checks that a dish id exists and belongs to one of the place's countries.
async fn valid_dish(state: &AppState, place_id: &str, dish: Option<&str>) -> Result<(), ApiError> {
    let Some(id) = dish else { return Ok(()) };
    let dish = catalog::dish(id).ok_or_else(|| ApiError::BadRequest(format!("unknown dish {id}")))?;
    if !store::place_countries(&state.db, place_id).await?.contains(&dish.country) {
        return Err(ApiError::BadRequest(format!("{} is not served here", dish.name)));
    }
    Ok(())
}

async fn visit_result(state: &AppState, user: &str, place_id: String, checkin_id: i64, verified: bool) -> ApiResult<VisitResult> {
    let profile = game::profile(store::lifetime(&state.db, user).await?);
    Ok(Json(VisitResult { place_id, checkin_id, verified, profile }))
}

/// A new check-in. Returning to a place counts again, once per cooldown period.
async fn check_in(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
    body: Option<Json<CheckIn>>,
) -> ApiResult<VisitResult> {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let verified = verify(&state, &place_id, &body).await?;
    valid_dish(&state, &place_id, body.dish.as_deref()).await?;
    let now = state.now();
    if let Some(last) = store::last_checkin_at(&state.db, &user, &place_id).await? {
        if now - last < CHECKIN_COOLDOWN_SECS {
            return Err(ApiError::Conflict("You already checked in here today.".into()));
        }
    }
    let id = store::add_checkin(&state.db, &user, &place_id, verified, body.dish.as_deref(), now).await?;
    visit_result(&state, &user, place_id, id, verified).await
}

/// Marks a place as visited: the first check-in only. Repeating it keeps the original
/// date but can upgrade the visit to verified. Kept for clients from before repeat visits.
async fn add_visit(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
    body: Option<Json<CheckIn>>,
) -> ApiResult<VisitResult> {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let verified = verify(&state, &place_id, &body).await?;
    if store::last_checkin_at(&state.db, &user, &place_id).await?.is_some() {
        if verified {
            store::verify_latest_checkin(&state.db, &user, &place_id).await?;
        }
        let id = store::latest_checkin_id(&state.db, &user, &place_id).await?;
        return visit_result(&state, &user, place_id, id, verified).await;
    }
    let id = store::add_checkin(&state.db, &user, &place_id, verified, None, state.now()).await?;
    visit_result(&state, &user, place_id, id, verified).await
}

/// Removes every check-in at a place.
async fn remove_visits(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    store::remove_visits(&state.db, &user, &place_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct CheckinUpdate {
    /// The signature dish eaten, or null to clear it.
    dish: Option<String>,
}

async fn update_checkin(
    State(state): State<AppState>,
    User(user): User,
    Path(id): Path<i64>,
    Json(body): Json<CheckinUpdate>,
) -> ApiResult<game::Profile> {
    let place_id = store::checkin_place(&state.db, &user, id).await?.ok_or(ApiError::NotFound("check-in"))?;
    valid_dish(&state, &place_id, body.dish.as_deref()).await?;
    store::set_checkin_dish(&state.db, &user, id, body.dish.as_deref()).await?;
    Ok(Json(game::profile(store::lifetime(&state.db, &user).await?)))
}

async fn remove_checkin(
    State(state): State<AppState>,
    User(user): User,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    if store::remove_checkin(&state.db, &user, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("check-in"))
    }
}

async fn push_key(State(state): State<AppState>) -> ApiResult<serde_json::Value> {
    let key = state.push_key.clone().ok_or(ApiError::NotFound("push"))?;
    Ok(Json(json!({ "public_key": key })))
}

/// A browser's PushSubscription (as from `subscription.toJSON()`), plus where the user is.
#[derive(Deserialize)]
struct SubscribeBody {
    endpoint: String,
    keys: SubscribeKeys,
    lat: f64,
    lng: f64,
    radius_km: f64,
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
    AreaQuery { lat: body.lat, lng: body.lng, radius_km: Some(body.radius_km) }.validate()?;
    if body.timezone.parse::<chrono_tz::Tz>().is_err() {
        return Err(ApiError::BadRequest("unknown time zone".into()));
    }
    let subscription = crate::push::Subscription {
        endpoint: body.endpoint,
        p256dh: body.keys.p256dh,
        auth: body.keys.auth,
        lat: body.lat,
        lng: body.lng,
        radius_km: body.radius_km,
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
struct GeocodeQuery {
    q: String,
}

async fn geocode(State(state): State<AppState>, Query(query): Query<GeocodeQuery>) -> ApiResult<Vec<GeoResult>> {
    let q = query.q.trim();
    if q.len() < 2 {
        return Ok(Json(Vec::new()));
    }
    state.geocoder.search(q).await.map(Json).map_err(ApiError::Upstream)
}

#[derive(Deserialize)]
struct ReverseQuery {
    lat: f64,
    lng: f64,
}

async fn reverse_geocode(
    State(state): State<AppState>,
    Query(query): Query<ReverseQuery>,
) -> ApiResult<serde_json::Value> {
    let name = state.geocoder.reverse(query.lat, query.lng).await.map_err(ApiError::Upstream)?;
    Ok(Json(json!({ "name": name })))
}
