//! Signing in with Apple, Google or an emailed code.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{ApiError, ApiResult, ErrorBody, JsonBody, MaybeSession, Session};
use crate::{account, auth, AppState};

/// The sign-in methods this server offers, with the public ids the web app needs.
#[derive(Serialize, ToSchema)]
pub(super) struct AuthProviders {
    /// The Services ID for Sign in with Apple on the web; `null` when it isn't set up
    /// (the iOS app signs in with Apple regardless).
    #[schema(required = true)]
    apple: Option<String>,
    /// The web client id for Google's sign-in button; `null` when Google isn't set up.
    #[schema(required = true)]
    google: Option<String>,
    /// Codes by email; always offered.
    email: bool,
}

#[utoipa::path(get, path = "/api/auth/providers", tag = "account", responses((status = 200, body = AuthProviders)))]
pub(super) async fn auth_providers(State(state): State<AppState>) -> Json<AuthProviders> {
    Json(AuthProviders {
        apple: state.auth.config.apple_web_client_id.clone(),
        google: state.auth.config.google_web_client_id.clone(),
        email: true,
    })
}

#[derive(Deserialize, ToSchema)]
pub(super) struct IdTokenSignIn {
    /// The ID token (a JWT) from Sign in with Apple or Google.
    id_token: String,
    /// As in `POST /api/session`.
    #[serde(default)]
    device: Option<String>,
}

async fn sign_in_provider(
    state: &AppState,
    provider: auth::Provider,
    current: Option<account::Session>,
    body: IdTokenSignIn,
) -> ApiResult<Session> {
    let who = state
        .auth
        .verify(provider, &body.id_token, state.now())
        .await?;
    let token = account::sign_in_with(
        &state.db,
        provider.name(),
        &who.subject,
        &account::Profile {
            email: who.email.as_deref(),
            name: who.name.as_deref(),
            picture: who.picture.as_deref(),
        },
        current.as_ref(),
        body.device.as_deref(),
        state.now(),
    )
    .await?;
    Ok(Json(Session { token }))
}

/// Signs in with Apple. Sent with the device's current token, an Apple ID new to the server
/// joins that account; one that already has an account brings this device there, and
/// what the device's account held moves along, as with a recovery code.
#[utoipa::path(
    post, path = "/api/auth/apple", tag = "account", request_body = IdTokenSignIn,
    security((), ("token" = [])),
    responses((status = 200, body = Session), (status = "default", description = "`bad_request` for a token that isn't valid", body = ErrorBody)),
)]
pub(super) async fn sign_in_apple(
    State(state): State<AppState>,
    MaybeSession(current): MaybeSession,
    JsonBody(body): JsonBody<IdTokenSignIn>,
) -> ApiResult<Session> {
    sign_in_provider(&state, auth::Provider::Apple, current, body).await
}

/// Signs in with Google, as `POST /api/auth/apple` does with Apple.
#[utoipa::path(
    post, path = "/api/auth/google", tag = "account", request_body = IdTokenSignIn,
    security((), ("token" = [])),
    responses((status = 200, body = Session), (status = "default", description = "`bad_request` for a token that isn't valid", body = ErrorBody)),
)]
pub(super) async fn sign_in_google(
    State(state): State<AppState>,
    MaybeSession(current): MaybeSession,
    JsonBody(body): JsonBody<IdTokenSignIn>,
) -> ApiResult<Session> {
    sign_in_provider(&state, auth::Provider::Google, current, body).await
}

#[derive(Deserialize, ToSchema)]
pub(super) struct EmailCodeRequest {
    email: String,
}

/// Emails a 6-digit sign-in code, valid for 10 minutes. A new one replaces the last.
#[utoipa::path(
    post, path = "/api/auth/email/code", tag = "account", request_body = EmailCodeRequest,
    responses(
        (status = 204, description = "Sent"),
        (status = 429, description = "`too_many_requests`: wait a few minutes", body = ErrorBody),
        (status = "default", description = "An error", body = ErrorBody),
    ),
)]
pub(super) async fn send_email_code(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<EmailCodeRequest>,
) -> Result<StatusCode, ApiError> {
    let email = auth::normalized_email(&body.email)
        .ok_or_else(|| ApiError::BadRequest("that isn't an email address".into()))?;
    let now = state.now();
    // Each mail costs, and nobody should flood an inbox: a few per address, and a cap for all.
    if !state.limits.allow(&email, "email_code", 3, 600, now)
        || !state.limits.allow("*", "email_code", 300, 3600, now)
    {
        return Err(ApiError::TooManyRequests(
            "too many codes asked for; try again in a few minutes",
        ));
    }
    let code = auth::new_code(&state.db, &email, now).await?;
    state
        .auth
        .send_code(&email, &code)
        .await
        .map_err(ApiError::Upstream)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub(super) struct EmailSignIn {
    email: String,
    /// The 6 digits from the email.
    code: String,
    /// As in `POST /api/session`.
    #[serde(default)]
    device: Option<String>,
}

/// Signs in with an emailed code, as `POST /api/auth/apple` does with Apple. A code works
/// once, for 10 minutes, and stops working after 5 wrong tries.
#[utoipa::path(
    post, path = "/api/auth/email", tag = "account", request_body = EmailSignIn,
    security((), ("token" = [])),
    responses(
        (status = 200, body = Session),
        (status = 404, description = "`not_found`: a wrong, used or expired code", body = ErrorBody),
        (status = "default", description = "An error", body = ErrorBody),
    ),
)]
pub(super) async fn sign_in_email(
    State(state): State<AppState>,
    MaybeSession(current): MaybeSession,
    JsonBody(body): JsonBody<EmailSignIn>,
) -> ApiResult<Session> {
    let email = auth::normalized_email(&body.email)
        .ok_or_else(|| ApiError::BadRequest("that isn't an email address".into()))?;
    if !auth::take_code(&state.db, &email, &body.code, state.now()).await? {
        return Err(ApiError::NotFound("code"));
    }
    let profile = account::Profile {
        email: Some(&email),
        ..Default::default()
    };
    let token = account::sign_in_with(
        &state.db,
        "email",
        &email,
        &profile,
        current.as_ref(),
        body.device.as_deref(),
        state.now(),
    )
    .await?;
    Ok(Json(Session { token }))
}
