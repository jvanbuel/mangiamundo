//! Request extractors: axum's, with JSON errors, and the caller's session.

use axum::extract::{FromRequest, FromRequestParts, OptionalFromRequest, Request};
use axum::http::header;
use axum::http::request::Parts;
use axum::Json;
use serde::de::DeserializeOwned;

use super::ApiError;
use crate::{account, AppState};

/// axum's `Query`, but a malformed query string is a JSON error like any other.
pub(super) struct Query<T>(pub(super) T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequestParts<S> for Query<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        let axum::extract::Query(value) =
            axum::extract::Query::from_request_parts(parts, state).await?;
        Ok(Query(value))
    }
}

/// axum's `Path`, but a path segment that does not decode is a JSON error like any other.
pub(super) struct Path<T>(pub(super) T);

impl<S: Send + Sync, T: DeserializeOwned + Send> FromRequestParts<S> for Path<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        let axum::extract::Path(value) =
            axum::extract::Path::from_request_parts(parts, state).await?;
        Ok(Path(value))
    }
}

/// axum's `Json` for request bodies, but a bad body is a JSON error like any other.
pub(super) struct JsonBody<T>(pub(super) T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for JsonBody<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, ApiError> {
        let Json(value) = <Json<T> as FromRequest<S>>::from_request(request, state).await?;
        Ok(JsonBody(value))
    }
}

/// No body (or no JSON content type) is `None`; a malformed JSON body is still an error.
impl<S: Send + Sync, T: DeserializeOwned> OptionalFromRequest<S> for JsonBody<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Option<Self>, ApiError> {
        let value = <Json<T> as OptionalFromRequest<S>>::from_request(request, state).await?;
        Ok(value.map(|Json(value)| JsonBody(value)))
    }
}

pub(super) type ApiResult<T> = Result<Json<T>, ApiError>;

/// The session behind the `Authorization: Bearer <token>` header, if there is one.
pub(super) struct MaybeSession(pub(super) Option<account::Session>);
/// Like [`MaybeSession`] but rejects anonymous requests.
pub(super) struct CurrentSession(pub(super) account::Session);
/// The user behind the `Authorization: Bearer <token>` header, if there is one.
pub(super) struct MaybeUser(pub(super) Option<String>);
/// Like [`MaybeUser`] but rejects anonymous requests.
pub(super) struct User(pub(super) String);

impl FromRequestParts<AppState> for MaybeSession {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let Some(value) = parts.headers.get(header::AUTHORIZATION) else {
            return Ok(MaybeSession(None));
        };
        let token = value
            .to_str()
            .ok()
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        match account::session_for_token(&state.db, token, state.now()).await? {
            Some(session) => Ok(MaybeSession(Some(session))),
            None => Err(ApiError::Unauthorized),
        }
    }
}

impl FromRequestParts<AppState> for CurrentSession {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        match MaybeSession::from_request_parts(parts, state).await? {
            MaybeSession(Some(session)) => Ok(CurrentSession(session)),
            MaybeSession(None) => Err(ApiError::Unauthorized),
        }
    }
}

impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let MaybeSession(session) = MaybeSession::from_request_parts(parts, state).await?;
        Ok(MaybeUser(session.map(|s| s.user)))
    }
}

impl FromRequestParts<AppState> for User {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let CurrentSession(session) = CurrentSession::from_request_parts(parts, state).await?;
        Ok(User(session.user))
    }
}
