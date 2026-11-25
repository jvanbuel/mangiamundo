//! Web Push: the server's key and browsers' subscriptions.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{ApiError, ApiResult, ErrorBody, JsonBody, User};
use crate::{areas, push, store, AppState};

#[derive(Serialize, ToSchema)]
pub(super) struct PushKey {
    /// The VAPID public key browsers subscribe with.
    public_key: String,
}

/// The key for Web Push. A 404 when the server has push turned off.
#[utoipa::path(get, path = "/api/push/key", tag = "push", responses((status = 200, body = PushKey), (status = "default", description = "An error", body = ErrorBody)))]
pub(super) async fn push_key(State(state): State<AppState>) -> ApiResult<PushKey> {
    let public_key = state.push_key.clone().ok_or(ApiError::NotFound("push"))?;
    Ok(Json(PushKey { public_key }))
}

/// A browser's `PushSubscription` (as from `subscription.toJSON()`), plus the user's area.
#[derive(Deserialize, ToSchema)]
pub(super) struct SubscribeBody {
    endpoint: String,
    keys: SubscribeKeys,
    /// The area to send reminders for.
    area: String,
    /// IANA name. Reminders go out from 10:00 in this zone.
    timezone: String,
}

#[derive(Deserialize, ToSchema)]
struct SubscribeKeys {
    p256dh: String,
    auth: String,
}

/// Subscribes a browser to reminders.
#[utoipa::path(
    put, path = "/api/push/subscription", tag = "push", request_body = SubscribeBody,
    security(("token" = [])),
    responses((status = 204, description = "Subscribed"), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn subscribe(
    State(state): State<AppState>,
    User(user): User,
    JsonBody(body): JsonBody<SubscribeBody>,
) -> Result<StatusCode, ApiError> {
    if state.push_key.is_none() {
        return Err(ApiError::NotFound("push"));
    }
    if !body.endpoint.starts_with("https://") && !body.endpoint.starts_with("http://127.0.0.1") {
        return Err(ApiError::BadRequest("endpoint must be an https URL".into()));
    }
    if body.timezone.parse::<chrono_tz::Tz>().is_err() {
        return Err(ApiError::BadRequest("unknown time zone".into()));
    }
    let subscription = push::Subscription {
        endpoint: body.endpoint,
        p256dh: body.keys.p256dh,
        auth: body.keys.auth,
        area_id: Some(areas::resolve(&state, &body.area).await?.id().to_string()),
        timezone: body.timezone,
    };
    store::save_push_subscription(&state.db, &user, &subscription, state.now()).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub(super) struct UnsubscribeBody {
    endpoint: String,
}

/// Unsubscribes a browser. A POST, because a DELETE with a body has no defined meaning
/// and some proxies drop it.
#[utoipa::path(
    post, path = "/api/push/subscription/delete", tag = "push", request_body = UnsubscribeBody,
    security(("token" = [])),
    responses((status = 204, description = "Unsubscribed"), (status = "default", description = "An error", body = ErrorBody)),
)]
pub(super) async fn unsubscribe(
    State(state): State<AppState>,
    User(user): User,
    JsonBody(body): JsonBody<UnsubscribeBody>,
) -> Result<StatusCode, ApiError> {
    store::remove_user_push_subscription(&state.db, &user, &body.endpoint).await?;
    Ok(StatusCode::NO_CONTENT)
}
