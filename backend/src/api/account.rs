//! The account: its devices, recovery and link codes, and deleting it.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

use super::{ApiError, ApiResult, CurrentSession, ErrorBody, Path, User};
use crate::{account, AppState};

/// The account: whether it has a recovery code, and the devices signed in to it.
#[utoipa::path(
    get, path = "/api/account", tag = "account", security(("token" = [])),
    responses((status = 200, body = account::Account), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn get_account(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
) -> ApiResult<account::Account> {
    Ok(Json(account::account(&state.db, &session).await?))
}

/// Deletes the account and everything in it: check-ins, added places, closed reports,
/// reminders, and the sessions of every device. It can't be undone.
#[utoipa::path(
    delete, path = "/api/account", tag = "account", security(("token" = [])),
    responses((status = 204, description = "Deleted"), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn delete_account(
    State(state): State<AppState>,
    User(user): User,
) -> Result<StatusCode, ApiError> {
    account::delete(&state.db, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub(super) struct RecoveryCode {
    /// `XXXX-XXXX-XXXX-XXXX-XXXX`. Shown this once: the server keeps only a hash.
    code: String,
}

/// Makes a new recovery code, which restores the account on any device through
/// `POST /api/session/restore`. The previous code stops working.
#[utoipa::path(
    post, path = "/api/account/recovery-code", tag = "account", security(("token" = [])),
    responses((status = 200, body = RecoveryCode), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn new_recovery_code(
    State(state): State<AppState>,
    User(user): User,
) -> ApiResult<RecoveryCode> {
    Ok(Json(RecoveryCode {
        code: account::new_recovery_code(&state.db, &user, state.now()).await?,
    }))
}

#[derive(Serialize, ToSchema)]
pub(super) struct LinkCode {
    /// `XXXX-XXXX`, to enter on the other device. It works once.
    code: String,
    expires_at: i64,
}

/// Makes a code that signs another device in to this account, through
/// `POST /api/session/restore`, within 10 minutes.
#[utoipa::path(
    post, path = "/api/account/link-code", tag = "account", security(("token" = [])),
    responses(
        (status = 200, body = LinkCode),
        (status = 429, description = "`too_many_requests`: more than 10 codes in an hour", body = ErrorBody),
        (status = "default", description = "An error", body = ErrorBody),
    ),
)]
pub(super) async fn new_link_code(
    State(state): State<AppState>,
    User(user): User,
) -> ApiResult<LinkCode> {
    let now = state.now();
    if !state.limits.allow(&user, "link_code", 10, 3600, now) {
        return Err(ApiError::TooManyRequests(
            "too many link codes, try again later",
        ));
    }
    let (code, expires_at) = account::new_link_code(&state.db, &user, now).await?;
    Ok(Json(LinkCode { code, expires_at }))
}

/// Signs a device out of the account. Signing out the current device ends this session.
#[utoipa::path(
    delete, path = "/api/account/devices/{device_id}", tag = "account",
    params(("device_id" = String, Path, description = "A device id from `GET /api/account`.")),
    security(("token" = [])),
    responses((status = 204, description = "Signed out"), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn remove_device(
    State(state): State<AppState>,
    User(user): User,
    Path(device_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if !account::remove_device(&state.db, &user, &device_id).await? {
        return Err(ApiError::NotFound("device"));
    }
    Ok(StatusCode::NO_CONTENT)
}
