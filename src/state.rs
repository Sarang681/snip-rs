use std::sync::Arc;
use fred::clients::Client;
use moka::future::Cache;
use sqlx::{Pool, Postgres};
use tokio::sync::mpsc::Sender;

use crate::{db, models::ClickEvent, redis};
use crate::snowflake::SnowflakeIdGenerator;

#[derive(Clone, Debug)]
pub struct AppState {
    pub conn_pool: Pool<Postgres>,
    pub redis_client: Option<Client>,
    pub event_sender: Sender<ClickEvent>,
    pub moka_cache: Cache<String, i64>,
    pub snowflake_id_generator: Arc<SnowflakeIdGenerator>
}

impl AppState {
    pub async fn new(
        db_url: &str,
        redis_url: &str,
        event_sender: Sender<ClickEvent>,
        moka_cache: Cache<String, i64>,
        snowflake_id_generator: Arc<SnowflakeIdGenerator>
    ) -> Self {
        let conn_pool = db::connection_pool(db_url).await;
        let redis_client = redis::get_redis_client(redis_url).await.ok();

        match &redis_client {
            Some(_) => tracing::info!("Redis connected"),
            None => tracing::warn!("Redis connection failed, running without redis"),
        }

        AppState {
            conn_pool,
            redis_client,
            event_sender,
            moka_cache,
            snowflake_id_generator
        }
    }
}
