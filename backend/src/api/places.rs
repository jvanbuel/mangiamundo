//! Places in an area, one place, and a suggestion of where to go.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{ApiError, ApiResult, ErrorBody, MaybeUser, Path, Query};
use crate::providers::osm;
use crate::store::{self, HoursStatus, PlaceView};
use crate::{areas, game, geo, hours, AppState};

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct PlacesQuery {
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
pub(super) async fn places(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Query(query): Query<PlacesQuery>,
) -> ApiResult<Vec<PlaceView>> {
    let area = areas::resolve(&state, &query.area).await?;
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
pub(super) fn complete(place: &mut PlaceView, from_lat: f64, from_lng: f64, now: i64) {
    place.osm_url = osm::link(&place.id);
    place.distance_m = geo::rounded_m(from_lat, from_lng, place.lat, place.lng);
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

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct SuggestionQuery {
    /// As for `/api/discover`.
    area: String,
    /// As for `/api/places`.
    lat: Option<f64>,
    lng: Option<f64>,
    /// Comma-separated place ids not to suggest, such as the ones spun before.
    exclude: Option<String>,
    /// `false` also suggests places that are closed now or have unknown hours. Defaults to `true`.
    open: Option<bool>,
}

/// One restaurant to try and the country the globe lands on.
#[derive(Serialize, ToSchema)]
pub(super) struct Suggestion {
    /// One of the place's `countries`.
    country: String,
    place: PlaceView,
}

/// A random place in the area the user hasn't checked in at, open now unless `open=false`.
/// Countries not tasted yet and nearer places are more likely. A 404 with code
/// `nothing_open` or `nothing_left` when nothing qualifies.
#[utoipa::path(
    get, path = "/api/suggestion", tag = "game", params(SuggestionQuery),
    security((), ("token" = [])),
    responses((status = 200, body = Suggestion), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn suggestion(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Query(query): Query<SuggestionQuery>,
) -> ApiResult<Suggestion> {
    let area = areas::resolve(&state, &query.area).await?;
    let exclude = query.exclude.unwrap_or_default();
    let exclude: Vec<&str> = exclude.split(',').collect();
    let tasted = match &user {
        Some(user) => store::visited_countries(&state.db, user).await?,
        None => Vec::new(),
    };
    let info = &area.boundary.info;
    let (from_lat, from_lng) = (query.lat.unwrap_or(info.lat), query.lng.unwrap_or(info.lng));
    let now = state.now();
    let mut unvisited: Vec<PlaceView> =
        store::area_places(&state.db, area.id(), None, user.as_deref())
            .await?
            .into_iter()
            .filter(|p| {
                p.visits.is_empty() && !p.countries.is_empty() && !exclude.contains(&p.id.as_str())
            })
            .collect();
    if unvisited.is_empty() {
        return Err(ApiError::NothingLeft);
    }
    for place in &mut unvisited {
        complete(place, from_lat, from_lng, now);
    }
    if query.open.unwrap_or(true) {
        unvisited.retain(|p| group(p) == 0);
        if unvisited.is_empty() {
            return Err(ApiError::NothingOpen);
        }
    }
    let candidates: Vec<game::Candidate> = unvisited
        .iter()
        .map(|p| game::Candidate {
            countries: &p.countries,
            distance_m: p.distance_m,
        })
        .collect();
    let (index, country) = game::suggest(&candidates, &tasted).ok_or(ApiError::NothingLeft)?;
    let mut place = unvisited.swap_remove(index);
    place.countries.sort_by_key(|c| *c != country);
    Ok(Json(Suggestion { country, place }))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct PlaceQuery {
    /// Where the user is, for the distance. Without it the distance is 0.
    pub(super) lat: Option<f64>,
    pub(super) lng: Option<f64>,
}

/// One place with all of the user's visits, newest first. Works for any place id, whatever
/// area it is in, so links such as `?place=osm-node-123` can open it directly.
#[utoipa::path(
    get, path = "/api/places/{place_id}", tag = "game",
    params(("place_id" = String, Path, description = "A place id from `/api/places`."), PlaceQuery),
    security((), ("token" = [])),
    responses((status = 200, body = PlaceView), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn place(
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
