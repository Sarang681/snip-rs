use axum::{http::StatusCode, response::IntoResponse};

pub enum AppError {
    BadUrlError,
    NotFoundError,
    DatabaseError(sqlx::Error),
    Gone,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AppError::BadUrlError => (StatusCode::BAD_REQUEST, "Invalid URL").into_response(),
            AppError::NotFoundError => (StatusCode::NOT_FOUND, "URL not found").into_response(),
            AppError::DatabaseError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong").into_response()
            }
            AppError::Gone => (
                StatusCode::GONE,
                "The requested resource is permanently gone, and no longer available",
            )
                .into_response(),
        }
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
