//! Checking in, removing a check-in, and reporting a place closed.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{zone, ApiError, ApiResult, ErrorBody, JsonBody, Path, Query, User};
use crate::progress::{self, Progress, VisitChange};
use crate::providers::osm;
use crate::store::{self, Visit};
use crate::{areas, calendar, game, geo, AppState};

/// A check-in counts as verified when made within this distance of the place.
const VERIFY_DISTANCE_M: f64 = 250.0;

#[derive(Deserialize, ToSchema)]
struct LatLng {
    lat: f64,
    lng: f64,
}

#[derive(Deserialize, Default, ToSchema)]
pub(super) struct CheckIn {
    /// Where the user is right now. Optional; used to mark the visit as verified.
    position: Option<LatLng>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct VisitQuery {
    /// As for `/api/discover`: the check-in counts for events by its date in this zone.
    tz: Option<String>,
    /// The area the user is playing in. With it, the answer has the progress there after
    /// the change (and for a check-in, what it unlocked), so there is no need to ask
    /// `/api/discover` again.
    area: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct VisitResult {
    visit: Visit,
    /// After the check-in, with the XP of the quests it finished.
    profile: game::Profile,
    /// The progress in `area` after the check-in; `null` without `area`.
    #[schema(required = true)]
    progress: Option<Progress>,
    /// What the check-in unlocked in `area`, in the order to announce it. Empty without
    /// `area`, and for a second check-in at a place.
    unlocked: Vec<progress::Unlocked>,
}

/// Checks in at a place. There is at most one visit per place per day: checking in again the
/// same day returns that day's visit, verified if either check-in was made on the spot.
///
/// Monthly quests belong to an area and are paid here, with or without `area`: in every
/// area the place is in, the quests this check-in finished add their XP to the profile.
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
pub(super) async fn add_visit(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
    Query(query): Query<VisitQuery>,
    body: Option<JsonBody<CheckIn>>,
) -> ApiResult<VisitResult> {
    let (_, plat, plng) = store::place_summary(&state.db, &place_id, &user)
        .await?
        .ok_or(ApiError::NotFound("place"))?;
    // Before anything is written, so a wrong area leaves no visit behind.
    let area = match &query.area {
        Some(id) => Some(areas::resolve(&state, id).await?),
        None => None,
    };
    // One moment for the whole check-in, read after the area: fetching it can take a while,
    // and the visit and the progress around it must agree on the day.
    let now = state.now();
    let today = calendar::local_date(now, zone(query.tz.as_deref()));
    let before = match &area {
        Some(area) => Some(progress::load(&state, Some(&user), area.id(), today).await?),
        None => None,
    };
    let check_in = body.map_or_default(|JsonBody(b)| b);
    let verified = check_in
        .position
        .is_some_and(|p| geo::distance_m(p.lat, p.lng, plat, plng) <= VERIFY_DISTANCE_M);
    let visit = store::add_visit(&state.db, &user, &place_id, verified, now, today).await?;
    // Before the progress and the profile are read, so both have the XP of the quests
    // this check-in finished.
    progress::settle_quests(&state, &user, &place_id, today, VisitChange::Added).await?;
    let (progress, unlocked) = match (&area, before) {
        (Some(area), Some(before)) => {
            let after = progress::load(&state, Some(&user), area.id(), today).await?;
            let unlocked = progress::unlocked(&before, &after, &area.boundary.info.name);
            (Some(after), unlocked)
        }
        _ => (None, Vec::new()),
    };
    let profile = game::profile(store::lifetime(&state.db, &user, today).await?);
    Ok(Json(VisitResult {
        visit,
        profile,
        progress,
        unlocked,
    }))
}

#[derive(Serialize, ToSchema)]
pub(super) struct VisitRemoved {
    /// The user's visits to the place that are left, newest first. The place stays visited
    /// while there are any.
    visits: Vec<Visit>,
    /// After the removal, without the XP of the quests that are no longer finished.
    profile: game::Profile,
    /// The progress in `area` after the removal; `null` without `area`.
    #[schema(required = true)]
    progress: Option<Progress>,
}

/// Removes one visit. The place stays visited while the user has other visits to it. The
/// XP of quests that are no longer finished without it is taken back, in every area.
#[utoipa::path(
    delete, path = "/api/visits/{visit_id}", tag = "game",
    params(("visit_id" = i64, Path, description = "A visit id from a place's `visits`."), VisitQuery),
    security(("token" = [])),
    responses((status = 200, body = VisitRemoved), (status = "default", description = "An error, `not_found` when the user has no such visit", body = ErrorBody)),
)]
pub(super) async fn remove_visit(
    State(state): State<AppState>,
    User(user): User,
    Path(visit_id): Path<i64>,
    Query(query): Query<VisitQuery>,
) -> ApiResult<VisitRemoved> {
    // Before anything is removed, so a wrong area leaves the visit in place.
    let area = match &query.area {
        Some(id) => Some(areas::resolve(&state, id).await?),
        None => None,
    };
    let (place_id, local_date) = store::remove_visit(&state.db, &user, visit_id)
        .await?
        .ok_or(ApiError::NotFound("visit"))?;
    if let Ok(day) = local_date.parse() {
        progress::settle_quests(&state, &user, &place_id, day, VisitChange::Removed).await?;
    }
    let today = state.today(zone(query.tz.as_deref()));
    let progress = match &area {
        Some(area) => Some(progress::load(&state, Some(&user), area.id(), today).await?),
        None => None,
    };
    Ok(Json(VisitRemoved {
        visits: store::visits(&state.db, &user, &place_id).await?,
        profile: game::profile(store::lifetime(&state.db, &user, today).await?),
        progress,
    }))
}

#[derive(Serialize, ToSchema)]
pub(super) struct ClosedResult {
    /// The progress in `area` without the place; `null` without `area`.
    #[schema(required = true)]
    progress: Option<Progress>,
}

/// "This place has closed": hides it for the reporter now, and for everyone after more reports.
#[utoipa::path(
    put, path = "/api/places/{place_id}/closed", tag = "game",
    params(("place_id" = String, Path, description = "A place id from `/api/places`."), VisitQuery),
    security(("token" = [])),
    responses((status = 200, body = ClosedResult), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn report_closed(
    State(state): State<AppState>,
    User(user): User,
    Path(place_id): Path<String>,
    Query(query): Query<VisitQuery>,
) -> ApiResult<ClosedResult> {
    let (name, lat, lng) = store::place_summary(&state.db, &place_id, &user)
        .await?
        .ok_or(ApiError::NotFound("place"))?;
    // Before the report, so a wrong area reports nothing.
    let area = match &query.area {
        Some(id) => Some(areas::resolve(&state, id).await?),
        None => None,
    };
    let reports = store::report_closed(&state.db, &user, &place_id, state.now()).await?;
    // Tell OSM once, when the place gets hidden for everyone, so one person cannot spam mappers.
    if reports == Some(2) {
        osm::note_closed(&state, &place_id, &name, lat, lng);
    }
    let progress = match &area {
        Some(area) => {
            let today = state.today(zone(query.tz.as_deref()));
            Some(progress::load(&state, Some(&user), area.id(), today).await?)
        }
        None => None,
    };
    Ok(Json(ClosedResult { progress }))
}
