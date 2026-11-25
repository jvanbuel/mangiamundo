//! Sharing progress and check-ins as public links.

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

use super::{zone, ApiError, ApiResult, AreaQuery, ErrorBody, Path, Query, User};
use crate::{areas, game, share, store, AppState};

/// A public link and what to say alongside it.
#[derive(Serialize, ToSchema)]
pub(super) struct ShareLink {
    /// The public page, also the link preview.
    url: String,
    /// The card image: 1200 × 630 PNG, the map with a caption.
    image_url: String,
    /// A sentence to share with the link.
    text: String,
}

/// `https://mangiamundo.com`: where the request was sent, as the browser or app saw it.
pub(super) fn origin(headers: &header::HeaderMap) -> String {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    let local = host.starts_with("localhost")
        || host.starts_with("127.0.0.1")
        || host.starts_with("10.0.2.2");
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .unwrap_or(if local { "http" } else { "https" });
    format!("{proto}://{host}")
}

pub(super) fn host(headers: &header::HeaderMap) -> String {
    origin(headers)
        .split("://")
        .nth(1)
        .unwrap_or_default()
        .to_string()
}

/// Shares the user's progress in an area: makes their public page (the first time) or
/// moves it to this area, and returns its link. `DELETE` turns the page off.
#[utoipa::path(
    post, path = "/api/share", tag = "share", params(AreaQuery), security(("token" = [])),
    responses((status = 200, body = ShareLink), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn share_profile(
    State(state): State<AppState>,
    User(user): User,
    headers: header::HeaderMap,
    Query(query): Query<AreaQuery>,
) -> ApiResult<ShareLink> {
    let area = areas::resolve(&state, &query.area).await?;
    let slug = share::publish_profile(&state.db, &user, area.id(), state.now()).await?;
    let rows = store::area_rows(&state.db, area.id(), Some(&user)).await?;
    let today = state.today(zone(query.tz.as_deref()));
    let summary = game::summarize(&rows, store::lifetime(&state.db, &user, today).await?);
    let url = format!("{}/p/{slug}", origin(&headers));
    Ok(Json(ShareLink {
        image_url: format!("{url}/card.png"),
        url,
        text: share::profile_text(&summary, &area.boundary.info.name),
    }))
}

/// Turns the public page off: its link stops working. Sharing again makes a new one.
#[utoipa::path(
    delete, path = "/api/share", tag = "share", security(("token" = [])),
    responses((status = 204, description = "Off, or it was never on"), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn unshare_profile(
    State(state): State<AppState>,
    User(user): User,
) -> Result<StatusCode, ApiError> {
    share::unpublish_profile(&state.db, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Shares one check-in: a public page with the place, its food and the date. The same
/// link each time; it goes when the check-in is removed.
#[utoipa::path(
    post, path = "/api/visits/{visit_id}/share", tag = "share",
    params(("visit_id" = i64, Path, description = "A visit id from a place's `visits`.")),
    security(("token" = [])),
    responses((status = 200, body = ShareLink), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn share_visit(
    State(state): State<AppState>,
    User(user): User,
    headers: header::HeaderMap,
    Path(visit_id): Path<i64>,
) -> ApiResult<ShareLink> {
    let slug = share::share_visit(&state.db, &user, visit_id, state.now())
        .await?
        .ok_or(ApiError::NotFound("visit"))?;
    let visit = share::find_visit(&state.db, &slug)
        .await?
        .ok_or(ApiError::NotFound("visit"))?;
    let url = format!("{}/c/{slug}", origin(&headers));
    Ok(Json(ShareLink {
        image_url: format!("{url}/card.png"),
        url,
        text: format!("{} on Mangiamundo.", share::visit_headline(&visit)),
    }))
}
