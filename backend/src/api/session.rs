//! Anonymous sessions, and restoring an account with a code.

use axum::extract::State;
use axum::http::header;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{client_of, ApiError, ApiResult, ErrorBody, JsonBody, MaybeSession};
use crate::{account, AppState};

#[derive(Serialize, ToSchema)]
pub(super) struct Session {
    /// Send it as `Authorization: Bearer <token>`.
    pub(super) token: String,
}

#[derive(Deserialize, ToSchema)]
pub(super) struct NewSession {
    /// What the app calls this device in the account's device list, such as "iPhone",
    /// "Android" or "Web". At most 60 characters are kept.
    #[serde(default)]
    device: Option<String>,
}

/// The address a session is being made from, for [`create_session`]'s limit. `None`
/// without the proxy (development, tests), where addresses can't be told apart.
fn session_client(headers: &header::HeaderMap) -> Option<String> {
    Some(client_of(headers)).filter(|client| client != "local")
}

/// Creates an anonymous account and returns the token of its first device.
///
/// A device asks for one before its first write, not before: what it shows until then it
/// can read without a token. It keeps the token. At most 20 accounts an hour are made from
/// one address.
#[utoipa::path(
    post, path = "/api/session", tag = "account",
    request_body(content = Option<NewSession>, description = "Optional."),
    responses(
        (status = 200, body = Session),
        (status = 429, description = "`too_many_requests`: too many accounts were made from this address in the last hour", body = ErrorBody),
        (status = "default", description = "An error", body = ErrorBody),
    ),
)]
pub(super) async fn create_session(
    State(state): State<AppState>,
    headers: header::HeaderMap,
    body: Option<JsonBody<NewSession>>,
) -> ApiResult<Session> {
    let now = state.now();
    // Per address where the proxy tells it, and for everyone together, so that addresses
    // that can't be told apart (or a great many of them) don't fill the database either.
    let client = session_client(&headers);
    if client.is_some_and(|client| !state.limits.allow(&client, "session", 20, 3600, now))
        || !state.limits.allow("*", "session", 1000, 3600, now)
    {
        return Err(ApiError::TooManyRequests(
            "too many new accounts from here; try again in an hour",
        ));
    }
    let device = body.and_then(|JsonBody(b)| b.device);
    let token = account::create_user(&state.db, device.as_deref(), now).await?;
    Ok(Json(Session { token }))
}

#[derive(Deserialize, ToSchema)]
pub(super) struct Restore {
    /// A recovery code (`XXXX-XXXX-XXXX-XXXX-XXXX`) or a link code (`XXXX-XXXX`). Case,
    /// spaces and dashes don't matter.
    code: String,
    /// As in `POST /api/session`.
    #[serde(default)]
    device: Option<String>,
}

/// Signs this device in to the account a recovery code or link code belongs to.
///
/// Sent with the device's current token, that session ends, and when it belonged to
/// another account, that account's check-ins, added places and the rest move into the
/// restored account, which then replaces it. A link code works once.
#[utoipa::path(
    post, path = "/api/session/restore", tag = "account", request_body = Restore,
    security((), ("token" = [])),
    responses(
        (status = 200, body = Session),
        (status = 404, description = "`not_found`: an unknown, used or expired code", body = ErrorBody),
        (status = "default", description = "An error", body = ErrorBody),
    ),
)]
pub(super) async fn restore_session(
    State(state): State<AppState>,
    MaybeSession(current): MaybeSession,
    JsonBody(body): JsonBody<Restore>,
) -> ApiResult<Session> {
    let token = account::redeem(
        &state.db,
        &body.code,
        current.as_ref(),
        body.device.as_deref(),
        state.now(),
    )
    .await?
    .ok_or(ApiError::NotFound("code"))?;
    Ok(Json(Session { token }))
}
