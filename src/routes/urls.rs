use axum::{
    Json, Router,
    extract::{Path, State},
    response::Redirect,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{AppState, db, encode, error::AppError, redis};

#[derive(Deserialize, Debug)]
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

#[tracing::instrument(skip(state))]
async fn handle_shorten_url(
    State(state): State<AppState>,
    Json(request): Json<ShortenRequest>,
) -> Result<Json<ShortenResponse>, AppError> {
    validate_request_url(&request.long_url)?;
    let db_pool = &state.conn_pool;
    let id = db::add_url(&request.long_url, &db_pool).await?;
    let short_code = encode::encode(id);

    //insert the shortened url into redis
    insert_short_code_into_redis(&state, &short_code, &request.long_url).await;

    Ok(Json(ShortenResponse { short_code }))
}

#[tracing::instrument(skip(state))]
async fn handle_redirect_from_short_code(
    State(state): State<AppState>,
    Path(short_code): Path<String>,
) -> Result<Redirect, AppError> {
    if let Some(long_url) = fetch_long_url_from_redis(&state, &short_code).await {
        tracing::info!("Found url in redis, returning without making db call");
        return Ok(Redirect::temporary(&long_url));
    }
    let db_pool = &state.conn_pool;
    match encode::decode(&short_code) {
        Some(decoded_id) => {
            let long_url = db::fetch_url(decoded_id, &db_pool).await?;
            insert_short_code_into_redis(&state, &short_code, &long_url).await;
            Ok(Redirect::temporary(&long_url))
        }
        None => Err(AppError::BadUrlError),
    }
}

fn validate_request_url(url: &str) -> Result<url::Url, AppError> {
    let parsed_url = url::Url::parse(url)?;
    Ok(parsed_url)
}

async fn insert_short_code_into_redis(state: &AppState, short_code: &str, long_url: &str) {
    if let Some(client) = &state.redis_client {
        match redis::put_key(client, short_code, long_url).await {
            Ok(_) => tracing::info!("Inserted code :: {} successfully", short_code),
            Err(_) => tracing::warn!("Could not insert the short code into redis"),
        }
    }
}

async fn fetch_long_url_from_redis(state: &AppState, short_code: &str) -> Option<String> {
    if let Some(client) = &state.redis_client {
        match redis::get_key(client, short_code).await {
            Ok(result) => {
                return Some(result);
            }
            Err(_) => {
                tracing::warn!("Redis key not found");
                return None;
            }
        }
    }
    None
}
