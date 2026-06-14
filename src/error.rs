use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    ///A machine-readable error code.
    #[schema(value_type = String, example = "BadUrlError")]
    error: String,
    ///A human-readable description of the error.
    #[schema(value_type = String, example = "The provided URL is malformed or invalid.")]
    message: String,
}

pub enum AppError {
    BadUrlError,
    NotFoundError,
    DatabaseError(sqlx::Error),
    Gone,
    BadTimestampError,
    RateLimitedError,
    ServerError,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, error, message): (StatusCode, &str, &str) = match self {
            AppError::BadUrlError => (
                StatusCode::BAD_REQUEST,
                "BadUrlError",
                "The provided URL is malformed or invalid.",
            ),
            AppError::NotFoundError => (
                StatusCode::NOT_FOUND,
                "NotFoundError",
                "The requested short code does not exist.",
            ),
            AppError::DatabaseError(error) => {
                tracing::error!("Database error occured :: {}", error);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "InternalError",
                    "An unexpected error occurred while processing your request.",
                )
            }
            AppError::Gone => (
                StatusCode::GONE,
                "LinkExpired",
                "This link has expired and is no longer available.",
            ),
            AppError::BadTimestampError => (
                StatusCode::BAD_REQUEST,
                "BadTimestampError",
                "The provided expiration timestamp is invalid or out of range.",
            ),
            AppError::RateLimitedError => (
                StatusCode::TOO_MANY_REQUESTS,
                "RateLimitedError",
                "User has exceeded the limit of requests that can be made in a minute.",
            ),
            AppError::ServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ServerError",
                "Something went wrong",
            ),
        };

        let body = Json(ErrorResponse {
            error: error.to_string(),
            message: message.to_string(),
        });

        (status, body).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        match value {
            sqlx::Error::RowNotFound => AppError::NotFoundError,
            _ => AppError::DatabaseError(value),
        }
    }
}

impl From<url::ParseError> for AppError {
    fn from(_: url::ParseError) -> Self {
        AppError::BadUrlError
    }
}

impl From<time::error::ComponentRange> for AppError {
    fn from(_: time::error::ComponentRange) -> Self {
        AppError::BadTimestampError
    }
}
