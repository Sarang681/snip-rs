use std::net::SocketAddr;

use crate::{
    AppState,
    db::{self},
    encode,
    error::{AppError, ErrorResponse},
    models::{ClickEvent, FetchedLink, ShortenRequest, ShortenResponse},
    ratelimit::RateLimited,
    redis,
};
use axum::{
    Json, Router,
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
    response::Redirect,
    routing::{get, post},
};
use fred::error::ErrorKind;
use sqlx::types::time::OffsetDateTime;
use sqlx::{Pool, Postgres};
use tokio::sync::mpsc::Sender;
use tracing::{error, info};

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
    let id = state.snowflake_id_generator.generate()? as i64;
    info!("Generated ID :: {}", id);

    if let Some(value) =
        add_url_to_postgres_db(&state, &request, expiration_date, db_pool, id).await
    {
        return value;
    }
    info!("URL added to db successfully");
    let short_code = encode::encode(id as u64);

    info!("Generated short code successfully :: {}", short_code);
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
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
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
        info!("Found url in redis, returning without making db call");
        send_click_event(state.event_sender, click_event).await;
        return Ok(Redirect::temporary(&long_url));
    }
    let db_pool = &state.conn_pool;
    match encode::decode(&short_code) {
        Some(decoded_id) => {
            if !state.postgres_circuit_breaker.allow_request() {
                error!("Postgres service is down, circuit breaker terminating the request");
                return Err(AppError::ServerError);
            }
            match db::fetch_url(decoded_id, db_pool).await {
                Ok(result) => {
                    state.postgres_circuit_breaker.record_success();
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
                Err(e) => {
                    state.postgres_circuit_breaker.record_failure();
                    error!("Error while fetching URL from postgres DB :: {}", e);
                    return Err(AppError::ServerError);
                }
            }
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
        if !state.redis_circuit_breaker.allow_request() {
            error!("Redis service is down, circuit breaker terminating the request");
            return;
        }
        let ttl = get_cache_ttl(result.expiration_date);
        match redis::put_url_key(client, short_code, &result.long_url, ttl).await {
            Ok(_) => {
                info!("Inserted code :: {} successfully", short_code);
                state.redis_circuit_breaker.record_success();
            }
            Err(e) => {
                error!("Could not insert the short code into redis :: {}", e);
                state.redis_circuit_breaker.record_failure();
            }
        }
    } else {
        error!("Error while fetching redis client");
        state.redis_circuit_breaker.record_failure();
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
    if !state.redis_circuit_breaker.allow_request() {
        error!("Redis service is down, circuit breaker terminating the request");
        return None;
    }

    if let Some(client) = &state.redis_client {
        match redis::get_url_key(client, short_code).await {
            Ok(result) => {
                state.redis_circuit_breaker.record_success();
                Some(result)
            }
            Err(e) => match e.kind() {
                ErrorKind::NotFound => {
                    state.redis_circuit_breaker.record_success();
                    tracing::warn!("Redis key not found");
                    None
                }
                _ => {
                    state.redis_circuit_breaker.record_failure();
                    error!("Error while fetching URL from cache :: {}", e);
                    None
                }
            },
        }
    } else {
        error!("Error fetching redis client");
        state.redis_circuit_breaker.record_failure();
        None
    }
}

async fn send_click_event(event_sender: Sender<ClickEvent>, click_event: ClickEvent) {
    if let Err(e) = event_sender.try_send(click_event) {
        tracing::warn!("Receiver dropped :: {}", e);
    }
}

async fn add_url_to_postgres_db(
    state: &AppState,
    request: &ShortenRequest,
    expiration_date: Option<OffsetDateTime>,
    db_pool: &Pool<Postgres>,
    id: i64,
) -> Option<Result<Json<ShortenResponse>, AppError>> {
    if !state.postgres_circuit_breaker.allow_request() {
        error!("Postgres Service is down, circuit breaker terminating the request");
        return Some(Err(AppError::ServerError));
    }

    match db::add_url(id, &request.long_url, expiration_date, db_pool).await {
        Ok(()) => state.postgres_circuit_breaker.record_success(),
        Err(e) => {
            state.postgres_circuit_breaker.record_failure();
            error!("Unable to insert url into postgres db :: {}", e);
            return Some(Err(AppError::ServerError));
        }
    }
    None
}
