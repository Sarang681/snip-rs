use std::net::SocketAddr;

use axum::{
    Json, Router,
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
    response::Redirect,
    routing::{get, post},
};
use sqlx::types::time::OffsetDateTime;
use tokio::sync::mpsc::Sender;

use crate::{
    AppState,
    db::{self},
    encode,
    error::{AppError, ErrorResponse},
    models::{ClickEvent, FetchedLink, ShortenRequest, ShortenResponse},
    ratelimit::RateLimited,
    redis,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/shorten", post(handle_shorten_url))
        .route("/{code}", get(handle_redirect_from_short_code))
}

#[tracing::instrument(skip(state))]
#[utoipa::path(
    post,
    path = "/shorten",
    request_body = ShortenRequest,
    responses(
        (status = 201, description = "URL successfully shortened", body = ShortenResponse),
        (status = 400, description = "Invalid URL provided", body = ErrorResponse),
        (status = 500, description = "Something went wrong", body = ErrorResponse)
    )
)]
async fn handle_shorten_url(
    State(state): State<AppState>,
    _rate_limit: RateLimited,
    Json(request): Json<ShortenRequest>,
) -> Result<Json<ShortenResponse>, AppError> {
    validate_request_url(&request.long_url)?;
    let expiration_date = validate_and_extract_expiration_date(request.expiration_date)?;
    let db_pool = &state.conn_pool;
    let id = db::add_url(&request.long_url, expiration_date, &db_pool).await?;
    let short_code = encode::encode(id);

    Ok(Json(ShortenResponse { short_code }))
}

#[tracing::instrument(skip(state))]
#[utoipa::path(
    get,
    path="/{short_code}",
    responses(
        (status = 307, description = "Temporary redirect to the original long URL"),
        (status = 404, description = "Short code is invalid or does not exist in the database", body = ErrorResponse),
        (status = 410, description = "The link has expired and is no longer available", body = ErrorResponse) ,
        (status = 500, description = "Something went wrong", body = ErrorResponse)
    )
)]
async fn handle_redirect_from_short_code(
    State(state): State<AppState>,
    Path(short_code): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>, //TODO: use axum-client-ip crate to extract IP address
    headers: HeaderMap,
) -> Result<Redirect, AppError> {
    let ip_addr = addr.ip().to_string();
    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("Unknown")
        .to_string();
    let referrer = headers
        .get("referer")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("None")
        .to_string();

    let click_event = ClickEvent {
        short_code: short_code.clone(),
        ip_addr,
        user_agent,
        referrer,
        clicked_at: OffsetDateTime::now_utc(),
    };

    if let Some(long_url) = fetch_long_url_from_redis(&state, &short_code).await {
        tracing::info!("Found url in redis, returning without making db call");
        send_click_event(state.event_sender, click_event).await;
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
            send_click_event(state.event_sender, click_event).await;
            Ok(Redirect::temporary(&result.long_url))
        }
        None => Err(AppError::BadUrlError),
    }
}

fn validate_request_url(url: &str) -> Result<url::Url, AppError> {
    let parsed_url = url::Url::parse(url)?;
    Ok(parsed_url)
}

fn validate_and_extract_expiration_date(
    request_expiration_date: Option<i64>,
) -> Result<Option<OffsetDateTime>, AppError> {
    if let Some(expiration_date) = request_expiration_date {
        let result = OffsetDateTime::from_unix_timestamp(expiration_date)?;
        Ok(Some(result))
    } else {
        Ok(None)
    }
}

async fn insert_short_code_into_redis(state: &AppState, short_code: &str, result: &FetchedLink) {
    if let Some(client) = &state.redis_client {
        let ttl = get_cache_ttl(result.expiration_date);
        match redis::put_url_key(client, short_code, &result.long_url, ttl).await {
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
        match redis::get_url_key(client, short_code).await {
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

async fn send_click_event(event_sender: Sender<ClickEvent>, click_event: ClickEvent) {
    if let Err(e) = event_sender.try_send(click_event) {
        tracing::warn!("Receiver dropped :: {}", e);
    }
}
