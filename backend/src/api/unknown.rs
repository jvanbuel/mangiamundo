//! Restaurants OpenStreetMap gives no cuisine, and what users say they serve.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use super::{
    add, AddedPlace, ApiError, ApiResult, ErrorBody, JsonBody, MaybeUser, Path, Query, User,
};
use crate::providers::osm;
use crate::store;
use crate::{areas, catalog, geo, AppState};

/// At most this many restaurants with unknown food are listed.
const UNKNOWN_LISTED: usize = 30;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct UnknownQuery {
    /// As for `/api/discover`.
    area: String,
    /// As for `/api/places`.
    lat: Option<f64>,
    lng: Option<f64>,
}

/// Named restaurants in the area that OpenStreetMap gives no cuisine, nearest first, at most
/// 30. Only where the daily import reaches (Belgium and its border towns); elsewhere the list
/// is empty. Leaves out the ones the user already answered for, and the ones two users said
/// serve nothing specific.
#[utoipa::path(
    get, path = "/api/unknown-places", tag = "game", params(UnknownQuery),
    security((), ("token" = [])),
    responses((status = 200, body = Vec<store::UnknownPlace>), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn unknown_places(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Query(query): Query<UnknownQuery>,
) -> ApiResult<Vec<store::UnknownPlace>> {
    let area = areas::resolve(&state, &query.area).await?;
    let shape = &area.boundary.shape;
    let info = &area.boundary.info;
    let (lat, lng) = (query.lat.unwrap_or(info.lat), query.lng.unwrap_or(info.lng));
    let mut places = store::unknown_in(&state.db, shape.bbox(), user.as_deref()).await?;
    places.retain(|p| shape.contains(p.lat, p.lng));
    for p in &mut places {
        p.distance_m = geo::rounded_m(lat, lng, p.lat, p.lng);
    }
    places.sort_by_key(|p| p.distance_m);
    places.truncate(UNKNOWN_LISTED);
    Ok(Json(places))
}

/// The food a restaurant with unknown food serves.
#[derive(Deserialize, ToSchema)]
pub(super) struct SuggestCuisine {
    /// As for `POST /api/places`.
    country: String,
}

/// Says which country's food a restaurant from `/api/unknown-places` serves. Like adding a
/// place: it shows up right away for this user only, an OpenStreetMap note asks mappers to
/// tag its cuisine, it counts towards the 5 a day, and it earns its reward once
/// OpenStreetMap has that cuisine. If OpenStreetMap tags another cuisine, that wins: the
/// place keeps the user's visits and earns nothing.
#[utoipa::path(
    post, path = "/api/unknown-places/{place_id}/cuisine", tag = "game", request_body = SuggestCuisine,
    params(("place_id" = String, Path)),
    security(("token" = [])),
    responses((status = 200, body = AddedPlace), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn suggest_cuisine(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
    JsonBody(body): JsonBody<SuggestCuisine>,
) -> ApiResult<AddedPlace> {
    let unknown = store::unknown(&state.db, &place_id)
        .await?
        .ok_or(ApiError::NotFound("place"))?;
    let code = body.country.to_uppercase();
    let (country, cuisine) = catalog::country(&code)
        .zip(catalog::cuisine_tag(&code))
        .ok_or(ApiError::BadRequest("unknown country".into()))?;
    let note = osm::cuisine_note(&unknown, &country.name, cuisine);
    let new = store::NewPlace {
        name: unknown.name,
        lat: unknown.lat,
        lng: unknown.lng,
        address: unknown.address,
        url: None,
        country: &country.code,
        cuisine,
        suggested_for: Some(unknown.id),
    };
    add(&state, &user, new, Some(note)).await
}

/// Says a restaurant from `/api/unknown-places` serves no particular country's food, such
/// as a snack bar or a Belgian brasserie. It leaves this user's list, and everyone's once
/// two users said so. Nothing is sent to OpenStreetMap, which has no tag for that.
#[utoipa::path(
    put, path = "/api/unknown-places/{place_id}/no-cuisine", tag = "game",
    params(("place_id" = String, Path)),
    security(("token" = [])),
    responses((status = 204, description = "Recorded"), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn no_cuisine(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    store::unknown(&state.db, &place_id)
        .await?
        .ok_or(ApiError::NotFound("place"))?;
    store::report_no_cuisine(&state.db, &user, &place_id, state.now()).await?;
    Ok(StatusCode::NO_CONTENT)
}
