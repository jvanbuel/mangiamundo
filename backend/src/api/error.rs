//! Error responses: every failure is JSON with a status and a code clients can branch on.

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

use crate::providers::Refused;
use crate::{areas, auth};

/// Every error response, whatever the status.
#[derive(Serialize, ToSchema)]
pub(super) struct ErrorBody {
    /// For people. It may change, so clients branch on `code` instead.
    error: String,
    code: ErrorCode,
}

/// What went wrong, for clients that react to a specific case:
///
/// - `bad_request` (400, 413, 415, 422): invalid or missing parameters, or a malformed,
///   oversized (over 2 MiB) or non-JSON body.
/// - `unauthorized` (401): a missing or unknown session token.
/// - `not_found` (404): an unknown place, area, device, code or `/api` path.
/// - `method_not_allowed` (405): the path exists, but not with this method.
/// - `nothing_open` (404): `/api/suggestion` found unvisited places, but none open now;
///   ask again with `open=false`.
/// - `nothing_left` (404): `/api/suggestion` found no unvisited place at all.
/// - `too_many_requests` (429): a limit was reached, such as places added per day or
///   location lookups per minute.
/// - `upstream` (502): the places or areas provider failed.
/// - `internal` (500): anything else, such as a database error.
#[derive(Serialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    BadRequest,
    Unauthorized,
    NotFound,
    MethodNotAllowed,
    NothingOpen,
    NothingLeft,
    TooManyRequests,
    Upstream,
    Internal,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    /// A path, query string or body that axum could not parse, with axum's status
    /// (400, 413, 415 or 422).
    #[error("{1}")]
    Rejected(StatusCode, String),
    #[error("method not allowed")]
    MethodNotAllowed,
    #[error("missing or invalid session token")]
    Unauthorized,
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    TooManyRequests(&'static str),
    #[error("no unvisited place is open now")]
    NothingOpen,
    #[error("no unvisited place left in the area")]
    NothingLeft,
    #[error("the places provider is unavailable")]
    Upstream(#[source] anyhow::Error),
    #[error("internal error")]
    Database(#[from] sqlx::Error),
    #[error("internal error")]
    Internal(#[source] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, ErrorCode::BadRequest),
            ApiError::Rejected(status, _) => (*status, ErrorCode::BadRequest),
            ApiError::MethodNotAllowed => {
                (StatusCode::METHOD_NOT_ALLOWED, ErrorCode::MethodNotAllowed)
            }
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, ErrorCode::Unauthorized),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, ErrorCode::NotFound),
            ApiError::NothingOpen => (StatusCode::NOT_FOUND, ErrorCode::NothingOpen),
            ApiError::NothingLeft => (StatusCode::NOT_FOUND, ErrorCode::NothingLeft),
            ApiError::TooManyRequests(_) => {
                (StatusCode::TOO_MANY_REQUESTS, ErrorCode::TooManyRequests)
            }
            ApiError::Upstream(error) => {
                tracing::error!(error = format!("{error:#}"), "provider request failed");
                (StatusCode::BAD_GATEWAY, ErrorCode::Upstream)
            }
            ApiError::Database(error) => {
                tracing::error!(%error, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::Internal)
            }
            ApiError::Internal(error) => {
                tracing::error!(error = format!("{error:#}"), "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::Internal)
            }
        };
        (
            status,
            Json(ErrorBody {
                error: self.to_string(),
                code,
            }),
        )
            .into_response()
    }
}

/// A provider's failure. When it only refused to ask its server, the client can wait
/// that out.
pub(super) fn provider_error(error: anyhow::Error) -> ApiError {
    match error.downcast_ref::<Refused>() {
        Some(Refused::TooMany) => {
            ApiError::TooManyRequests("too many location lookups; try again in a few minutes")
        }
        Some(Refused::Busy) => {
            ApiError::TooManyRequests("location lookups are busy; try again in a moment")
        }
        None => ApiError::Upstream(error),
    }
}

impl From<areas::Error> for ApiError {
    fn from(error: areas::Error) -> Self {
        match error {
            areas::Error::Invalid | areas::Error::TooLarge(_) => {
                ApiError::BadRequest(error.to_string())
            }
            areas::Error::NotFound => ApiError::NotFound("area"),
            areas::Error::Upstream(error) => provider_error(error),
            areas::Error::Database(error) => ApiError::Database(error),
        }
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        ApiError::Rejected(rejection.status(), rejection.body_text())
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        ApiError::Rejected(rejection.status(), rejection.body_text())
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        ApiError::Rejected(rejection.status(), rejection.body_text())
    }
}

impl From<auth::AuthError> for ApiError {
    fn from(error: auth::AuthError) -> Self {
        match error {
            // Not a 401: clients take that to mean their own session is gone.
            auth::AuthError::NotConfigured(_) | auth::AuthError::Invalid => {
                ApiError::BadRequest(error.to_string())
            }
            auth::AuthError::Upstream(_, source) => ApiError::Upstream(source),
        }
    }
}
