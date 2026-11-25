//! Reading places again from OpenStreetMap, one or all nearby.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{complete, ApiError, ApiResult, ErrorBody, Path, PlaceQuery, Query, User};
use crate::providers::{osm, Lookup};
use crate::store::{self, PlaceView};
use crate::{hours, AppState};

/// What a refresh from OpenStreetMap found.
#[derive(Serialize, ToSchema)]
pub(super) struct RefreshResult {
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
pub(super) async fn refresh_place(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
    Query(query): Query<PlaceQuery>,
) -> ApiResult<RefreshResult> {
    if osm::link(&place_id).is_none() {
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
pub(super) struct NearbyQuery {
    /// Where the user is.
    lat: f64,
    lng: f64,
}

/// What a nearby refresh found.
#[derive(Serialize, ToSchema)]
pub(super) struct NearbyResult {
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
pub(super) async fn refresh_nearby(
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
        matched: crate::count(matched),
    }))
}
