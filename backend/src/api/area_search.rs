//! Finding areas: search, suggestions, locating a point, and loading progress.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{provider_error, ApiError, ApiResult, ErrorBody, Path, Query};
use crate::providers::{AreaInfo, Located};
use crate::AppState;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct SearchQuery {
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
pub(super) async fn search_areas(
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
pub(super) struct SuggestQuery {
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
pub(super) async fn suggest_areas(
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
pub(super) struct LocateQuery {
    lat: f64,
    lng: f64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LoadProgress {
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
pub(super) async fn area_progress(
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
pub(super) async fn locate_area(
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
