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

use crate::game::{self, Lifetime, Summary};
use crate::providers::GeoResult;
use crate::store::{self, Area, PlaceView};
use crate::{catalog, geo, AppState};

/// A check-in counts as verified when made within this distance of the place.
const VERIFY_DISTANCE_M: f64 = 250.0;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(|| async { Json(json!({ "ok": true })) }))
        .route("/api/session", post(create_session))
        .route("/api/countries", get(countries))
        .route("/api/discover", get(discover))
        .route("/api/places", get(places))
        .route("/api/visits/{place_id}", put(add_visit).delete(remove_visit))
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

#[derive(Deserialize)]
struct AreaQuery {
    lat: f64,
    lng: f64,
    radius_km: Option<f64>,
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
async fn resolve_area(state: &AppState, query: &AreaQuery) -> Result<Area, ApiError> {
    let radius_m = query.validate()?;
    let (lat, lng) = store::snap(query.lat, query.lng);
    let provider = state.places.name();
    let key = store::area_key(provider, lat, lng, radius_m);

    let cached = store::find_area(&state.db, &key).await?;
    if let Some(area) = &cached {
        if store::now() - area.fetched_at < state.cache_ttl_secs {
            return Ok(area.clone());
        }
    }

    match state.places.search(lat, lng, radius_m).await {
        Ok(found) => {
            let area = Area { key, lat, lng, radius_m, provider: provider.into(), fetched_at: store::now() };
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
    Ok(Json(Discover { summary: game::summarize(&rows, lifetime), area }))
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
    for place in &mut places {
        place.distance_m = geo::distance_m(query.lat, query.lng, place.lat, place.lng).round() as u32;
    }
    places.sort_by_key(|p| p.distance_m);
    Ok(Json(places))
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
    store::add_visit(&state.db, &user, &place_id, verified).await?;
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
