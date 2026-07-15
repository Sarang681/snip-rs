use std::net::SocketAddr;

use crate::{error::AppError, redis, state::AppState};
use axum::extract::{ConnectInfo, FromRef, FromRequestParts};
use tracing::error;

#[derive(Debug)]
pub struct RateLimited;

impl<S> FromRequestParts<S> for RateLimited
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let ip_addr = ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::ServerError)?
            .ip()
            .to_string();

        let state = AppState::from_ref(state);
        let hash_ring = &state.hash_ring;

        // TODO: Implement per-node circuit breakers
        // Currently using a single circuit breaker for all Redis nodes.
        // This means if one node fails repeatedly, the circuit trips and ALL nodes
        // become unavailable, even healthy ones.
        //
        // Future improvement:
        // - Create a HashMap<String, CircuitBreaker> mapping node_id -> circuit breaker
        // - Update ConsistentHashRing to return (Client, CircuitBreaker) pairs
        // - Each node gets independent fault tolerance
        if !state.redis_circuit_breaker.allow_request() {
            error!("Redis service is down, rate limiter using moka to limit incoming requests");
            rate_limit_moka(&ip_addr, &state).await?;
            return Ok(Self);
        }

        let hash_key = format!("ratelimit:{}:{}", ip_addr, "create_link");
        let client = hash_ring.get_client(&hash_key);

        if let Some(client) = client {
            match redis::put_rate_limit_key(&client, hash_key).await {
                Ok(value) => {
                    state.redis_circuit_breaker.record_success();
                    if value > 5 {
                        return Err(AppError::RateLimitedError);
                    }
                }
                Err(e) => {
                    error!("Error while rate limiting using redis :: {}", e);
                    state.redis_circuit_breaker.record_failure();
                    rate_limit_moka(&ip_addr, &state).await?;
                }
            }
        } else {
            error!("Couldn't find redis client while rate limiting");
            state.redis_circuit_breaker.record_failure();
            rate_limit_moka(&ip_addr, &state).await?;
        }

        Ok(Self)
    }
}

async fn rate_limit_moka(ip_addr: &str, state: &AppState) -> Result<(), AppError> {
    let moka_cache = &state.moka_cache;
    let key = format!("ratelimit:{}:{}", ip_addr, "create_link");
    let current_count = moka_cache.get(&key).await;
    if let Some(current_count) = current_count {
        if current_count + 1 > 5 {
            return Err(AppError::RateLimitedError);
        }
        moka_cache.insert(key, current_count + 1).await;
    } else {
        moka_cache.insert(key, 1).await;
    }
    Ok(())
}
