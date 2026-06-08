use fred::clients::Client;
use sqlx::{Pool, Postgres};

use crate::{db, redis};

#[derive(Clone, Debug)]
pub struct AppState {
    pub conn_pool: Pool<Postgres>,
    pub redis_client: Option<Client>,
}

impl AppState {
    pub async fn new(db_url: &str, redis_url: &str) -> Self {
        let conn_pool = db::connection_pool(db_url).await;
        let redis_client = redis::get_redis_client(redis_url).await.ok();

        match &redis_client {
            Some(_) => tracing::info!("Redis connected"),
            None => tracing::warn!("Redis connection failed, running without redis"),
        }

        AppState {
            conn_pool,
            redis_client,
        }
    }
}
