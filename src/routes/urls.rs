use std::ops::Add;

use axum::{
    Json, Router,
    extract::{Path, State},
    response::Redirect,
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::types::time::OffsetDateTime;

use crate::{
    AppState,
    db::{self, FetchedLink},
    encode,
    error::AppError,
    redis,
};

#[derive(Deserialize, Debug)]
struct ShortenRequest {
    long_url: String,
    expiration_date: i64,
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
    let expiration_date = OffsetDateTime::from_unix_timestamp(request.expiration_date)
        .expect("Invalid expiration date");
    let db_pool = &state.conn_pool;
    let id = db::add_url(&request.long_url, expiration_date, &db_pool).await?;
    let short_code = encode::encode(id);

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
            let result = db::fetch_url(decoded_id, &db_pool).await?;
            if let Some(expiration_date) = result.expiration_date {
                let now = OffsetDateTime::now_utc();
                if now > expiration_date {
                    return Err(AppError::Gone);
                }
            }
            insert_short_code_into_redis(&state, &short_code, &result).await;
            Ok(Redirect::temporary(&result.long_url))
        }
        None => Err(AppError::BadUrlError),
    }
}

fn validate_request_url(url: &str) -> Result<url::Url, AppError> {
    let parsed_url = url::Url::parse(url)?;
    Ok(parsed_url)
}

async fn insert_short_code_into_redis(state: &AppState, short_code: &str, result: &FetchedLink) {
    if let Some(client) = &state.redis_client {
        let ttl = get_cache_ttl(result.expiration_date);
        match redis::put_key(client, short_code, &result.long_url, ttl).await {
            Ok(_) => tracing::info!("Inserted code :: {} successfully", short_code),
            Err(_) => tracing::warn!("Could not insert the short code into redis"),
        }
    }
}

fn get_cache_ttl(expiration_date: Option<OffsetDateTime>) -> i64 {
    let max_ttl_expiration_secs = 24 * 7 * 60 * 60; //1 week
    if let Some(db_expiration_date) = expiration_date {
        let ttl_expiration_in_secs =
            (db_expiration_date - OffsetDateTime::now_utc()).as_seconds_f32() as i64;
        std::cmp::min(max_ttl_expiration_secs, ttl_expiration_in_secs)
    } else {
        max_ttl_expiration_secs
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
