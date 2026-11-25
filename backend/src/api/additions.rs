//! Restaurants users add because OpenStreetMap lacks them.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{complete, ApiError, ApiResult, ErrorBody, JsonBody, User};
use crate::providers::osm;
use crate::store::{self, PlaceView};
use crate::{catalog, game, AppState};

/// Places a user may add per day, so the app never floods OpenStreetMap with notes.
const PLACES_PER_DAY: i64 = 5;

/// A restaurant missing from the map.
#[derive(Deserialize, ToSchema)]
pub(super) struct AddPlace {
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
pub(super) struct AddedPlace {
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
pub(super) async fn add_place(
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
    let new = store::NewPlace {
        name,
        lat: body.lat,
        lng: body.lng,
        address,
        url,
        country: &country.code,
        cuisine,
        suggested_for: None,
    };
    let note = (body.note != Some(false)).then(|| osm::missing_place_note(&new));
    add(&state, &user, new, note).await
}

/// Saves a pending place, posts `note` about it to OpenStreetMap, and returns it with its
/// reward.
pub(super) async fn add(
    state: &AppState,
    user: &str,
    new: store::NewPlace,
    note: Option<String>,
) -> ApiResult<AddedPlace> {
    let now = state.now();
    let added = store::add_place(&state.db, user, &new, now, PLACES_PER_DAY)
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

    if let Some(text) = note {
        osm::note_added(state, id.clone(), new.lat, new.lng, text);
    }
    let mut place = store::place(&state.db, &id, Some(user))
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
pub(super) async fn additions(
    State(state): State<AppState>,
    User(user): User,
) -> ApiResult<Vec<store::Addition>> {
    Ok(Json(store::additions(&state.db, &user).await?))
}

/// Additions the user was told about.
#[derive(Deserialize, ToSchema)]
pub(super) struct SeenAdditions {
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
pub(super) async fn mark_additions_seen(
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
