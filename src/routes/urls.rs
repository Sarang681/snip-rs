use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{AppState, db, encode};

#[derive(Deserialize)]
struct ShortenRequest {
    long_url: String,
}

#[derive(Serialize)]
struct ShortenResponse {
    short_code: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/shorten", post(handle_shorten_url))
        .route("/{code}", get(handle_redirect_from_short_code))
}

async fn handle_shorten_url(
    State(state): State<AppState>,
    Json(request): Json<ShortenRequest>,
) -> Json<ShortenResponse> {
    let db_pool = state.conn_pool;
    let id = db::add_url(&request.long_url, &db_pool).await;
    let short_code = encode::encode(id);

    Json(ShortenResponse { short_code })
}

async fn handle_redirect_from_short_code(
    State(state): State<AppState>,
    Path(short_code): Path<String>,
) -> String {
    let db_pool = state.conn_pool;

    let decoded_id = encode::decode(&short_code).unwrap();

    db::fetch_url(decoded_id, &db_pool).await
}
