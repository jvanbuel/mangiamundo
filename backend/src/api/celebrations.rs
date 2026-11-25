//! Celebrations and reminders coming up in an area.

use std::collections::HashMap;

use axum::extract::State;
use axum::Json;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{zone, ApiResult, ErrorBody, MaybeUser, Query};
use crate::calendar::{self, Celebration};
use crate::push::{self, Reminder};
use crate::{areas, progress, store, AppState};

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct CelebrationsQuery {
    /// As for `/api/discover`.
    area: String,
    /// How many days ahead to look: 0 to 366, 60 when missing.
    days: Option<i64>,
    /// As for `/api/discover`.
    tz: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct NearbyCountry {
    code: String,
    places: u32,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CelebrationView {
    #[serde(flatten)]
    celebration: Celebration,
    /// The celebration's countries that have places in the area.
    nearby: Vec<NearbyCountry>,
    /// Notification text for the first day, for apps that schedule reminders themselves.
    reminder: push::Notification,
}

/// Celebrations coming up for countries whose food can be found in the area, with
/// the reminder text.
#[utoipa::path(
    get, path = "/api/celebrations", tag = "calendar", params(CelebrationsQuery),
    responses((status = 200, body = Vec<CelebrationView>), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn celebrations(
    State(state): State<AppState>,
    Query(query): Query<CelebrationsQuery>,
) -> ApiResult<Vec<CelebrationView>> {
    let zone = zone(query.tz.as_deref());
    let area = areas::resolve(&state, &query.area).await?;
    let rows = store::area_rows(&state.db, area.id(), None).await?;
    let counts = progress::places_per_country(&rows);
    let days = query.days.unwrap_or(60).clamp(0, 366);
    Ok(Json(nearby_celebrations(state.today(zone), days, &counts)))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct RemindersQuery {
    /// As for `/api/discover`.
    area: String,
    /// How many days ahead to look, today included: 0 to 366, 60 when missing.
    days: Option<i64>,
    /// As for `/api/discover`.
    tz: Option<String>,
}

/// The reminders that become due in the area from today on, soonest first, for apps that
/// schedule their own notifications: the ones Web Push sends on those days. An event's
/// "last days" reminder depends on the user's visits; without a token it is as for someone
/// with no visits. Ask again after a check-in or when the area changes.
#[utoipa::path(
    get, path = "/api/reminders", tag = "calendar", params(RemindersQuery),
    security((), ("token" = [])),
    responses((status = 200, body = Vec<Reminder>), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn reminders(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Query(query): Query<RemindersQuery>,
) -> ApiResult<Vec<Reminder>> {
    let area = areas::resolve(&state, &query.area).await?;
    let today = state.today(zone(query.tz.as_deref()));
    let days = query.days.unwrap_or(60).clamp(0, 366);
    let plan = push::plan(&state, user.as_deref(), area.id(), today, days).await?;
    Ok(Json(plan))
}

fn nearby_celebrations(
    today: NaiveDate,
    days: i64,
    counts: &HashMap<&str, u32>,
) -> Vec<CelebrationView> {
    calendar::upcoming(today, days)
        .into_iter()
        .filter_map(|celebration| {
            let nearby: Vec<NearbyCountry> = celebration
                .countries
                .iter()
                .filter_map(|code| {
                    Some(NearbyCountry {
                        code: code.clone(),
                        places: *counts.get(code.as_str())?,
                    })
                })
                .collect();
            let reminder = push::celebration_notification(&celebration, counts)?;
            Some(CelebrationView {
                celebration,
                nearby,
                reminder,
            })
        })
        .collect()
}
