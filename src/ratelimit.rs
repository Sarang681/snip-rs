use std::net::SocketAddr;

use axum::extract::{ConnectInfo, FromRef, FromRequestParts};

use crate::{error::AppError, redis, state::AppState};

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
        let client = state.redis_client;

        if let Some(client) = client {
            if let Ok(value) = redis::put_rate_limit_key(&client, &ip_addr, "create_link").await {
                if value >= 5 {
                    return Err(AppError::RateLimitedError);
                }
            } else {
                return Err(AppError::ServerError);
            }
        } else {
            return Err(AppError::ServerError);
        }

        Ok(Self)
    }
}
