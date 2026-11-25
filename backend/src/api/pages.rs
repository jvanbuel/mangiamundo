//! The public pages of shared progress and check-ins, and their card images.

use std::collections::HashMap;

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono_tz::Tz;

use super::{host, origin, ApiError, Path};
use crate::game::{self, Summary};
use crate::{share, store, AppState};

/// A public page's progress: the summary and the area's name.
async fn public_profile(state: &AppState, slug: &str) -> Result<(Summary, String), ApiError> {
    let (user, area_id) = share::find_profile(&state.db, slug)
        .await?
        .ok_or(ApiError::NotFound("page"))?;
    let area = store::find_area(&state.db, &area_id)
        .await?
        .ok_or(ApiError::NotFound("page"))?;
    let rows = store::area_rows(&state.db, &area_id, Some(&user)).await?;
    // ponytail: the viewer's zone is unknown here; UTC can be a day off for the streak.
    let today = state.today(Tz::UTC);
    let summary = game::summarize(&rows, store::lifetime(&state.db, &user, today).await?);
    Ok((summary, area.boundary.info.name))
}

fn html(body: String) -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
}

/// People open these links in a browser, so a page that's gone says so in HTML.
pub(super) fn page_or_gone(result: Result<Response, ApiError>) -> Response {
    match result {
        Err(ApiError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            html(share::page(
                "This link is gone",
                "Whoever shared it turned it off.",
                "/icon.svg",
                "/",
                "<h1>This link is gone</h1>\n<p class=\"muted\">Whoever shared it turned it off or removed the check-in.</p>\n",
            )),
        )
            .into_response(),
        other => other.into_response(),
    }
}

fn png(bytes: Vec<u8>) -> Response {
    (
        // Not kept by caches: a card must stop showing once its share is turned off.
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response()
}

/// Draws a card off the async threads; it takes a few dozen milliseconds.
async fn render_card(
    statuses: HashMap<String, game::Status>,
    headline: String,
    caption: String,
) -> Result<Vec<u8>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let statuses: HashMap<&str, game::Status> =
            statuses.iter().map(|(c, s)| (c.as_str(), *s)).collect();
        share::card_png(&statuses, &headline, &caption)
    })
    .await
    .map_err(|e| ApiError::Internal(e.into()))?
    .map_err(ApiError::Internal)
}

pub(super) async fn profile_page(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    Path(slug): Path<String>,
) -> Result<Response, ApiError> {
    let (summary, area) = public_profile(&state, &slug).await?;
    let url = format!("{}/p/{slug}", origin(&headers));
    Ok(html(share::page(
        &share::profile_headline(&summary),
        &share::profile_text(&summary, &area),
        &format!("{url}/card.png"),
        &url,
        &share::profile_body(&summary, &area),
    )))
}

pub(super) async fn profile_card(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    Path(slug): Path<String>,
) -> Result<Response, ApiError> {
    let (summary, area) = public_profile(&state, &slug).await?;
    let statuses = summary
        .countries
        .iter()
        .map(|c| (c.code.clone(), c.status))
        .collect();
    let caption = format!(
        "{area} · Level {} · {}",
        summary.profile.level,
        host(&headers)
    );
    Ok(png(render_card(
        statuses,
        share::profile_headline(&summary),
        caption,
    )
    .await?))
}

pub(super) async fn visit_page(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    Path(slug): Path<String>,
) -> Result<Response, ApiError> {
    let visit = share::find_visit(&state.db, &slug)
        .await?
        .ok_or(ApiError::NotFound("page"))?;
    let url = format!("{}/c/{slug}", origin(&headers));
    let headline = share::visit_headline(&visit);
    Ok(html(share::page(
        &headline,
        &format!("{headline}, {}.", share::pretty_date(&visit.date)),
        &format!("{url}/card.png"),
        &url,
        &share::visit_body(&visit),
    )))
}

pub(super) async fn visit_card(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    Path(slug): Path<String>,
) -> Result<Response, ApiError> {
    let visit = share::find_visit(&state.db, &slug)
        .await?
        .ok_or(ApiError::NotFound("page"))?;
    // The check-in's countries light up gold, as freshly tasted.
    let statuses = visit
        .countries
        .iter()
        .map(|c| (c.clone(), game::Status::Discovered))
        .collect();
    let caption = format!("{} · {}", share::pretty_date(&visit.date), host(&headers));
    Ok(png(render_card(
        statuses,
        share::visit_headline(&visit),
        caption,
    )
    .await?))
}
