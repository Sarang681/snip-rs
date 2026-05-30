use axum::{
    Json, Router,
    extract::Path,
    routing::{get, post},
};
use serde::Deserialize;

use crate::AppState;

#[derive(Deserialize)]
struct ShortenRequest {
    long_url: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/shorten", post(handle_shorten_url))
        .route("/{code}", get(handle_redirect_from_short_code))
}

async fn handle_shorten_url(Json(request): Json<ShortenRequest>) -> String {
    format!("Shortening the following url :: {}", request.long_url)
}

async fn handle_redirect_from_short_code(Path(short_code): Path<String>) -> String {
    format!(
        "Redirecting to the long url from this url :: {}",
        short_code
    )
}
