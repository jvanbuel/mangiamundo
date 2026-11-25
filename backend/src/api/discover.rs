//! The game screen of an area.

use axum::extract::State;
use axum::Json;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{ApiResult, ErrorBody, MaybeUser, Query};
use crate::progress::{self, Progress};
use crate::providers::AreaInfo;
use crate::store::Area;
use crate::{areas, AppState};

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct AreaQuery {
    /// An area id from `/api/areas` or `/api/areas/locate`.
    pub(super) area: String,
    /// The user's IANA time zone (e.g. `Asia/Tokyo`), which decides which day "today"
    /// is for event and celebration dates. UTC when missing or unknown.
    pub(super) tz: Option<String>,
}

/// Parses the optional `tz` parameter. An unknown zone (a browser can report one the
/// tz database here lacks) falls back to UTC rather than failing the whole screen.
pub(super) fn zone(tz: Option<&str>) -> Tz {
    match tz {
        None | Some("") => Tz::UTC,
        Some(name) => name.parse().unwrap_or_else(|_| {
            tracing::warn!(tz = name, "unknown time zone, using UTC");
            Tz::UTC
        }),
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
        AreaView {
            info: area.boundary.info.clone(),
            fetched_at: area.fetched_at,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(super) struct Discover {
    area: AreaView,
    #[serde(flatten)]
    progress: Progress,
}

/// Countries available in the area, progress, badges, profile, events and monthly quests.
#[utoipa::path(
    get, path = "/api/discover", tag = "game",
    params(AreaQuery),
    // Anonymous requests work too; an unknown token is still a 401.
    security((), ("token" = [])),
    responses((status = 200, body = Discover), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn discover(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Query(query): Query<AreaQuery>,
) -> ApiResult<Discover> {
    let area = areas::resolve(&state, &query.area).await?;
    let today = state.today(zone(query.tz.as_deref()));
    let progress = progress::load(&state, user.as_deref(), area.id(), today).await?;
    Ok(Json(Discover {
        area: (&area).into(),
        progress,
    }))
}
