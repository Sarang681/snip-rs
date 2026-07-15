use moka::future::Cache;
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

use crate::circuit_breaker::CircuitBreaker;
use crate::consistent_hashing::ConsistentHashRing;
use crate::snowflake::SnowflakeIdGenerator;
use crate::{db, models::ClickEvent};

#[derive(Clone, Debug)]
pub struct AppState {
    pub conn_pool: Pool<Postgres>,
    pub hash_ring: Arc<ConsistentHashRing>,
    pub event_sender: Sender<ClickEvent>,
    pub moka_cache: Cache<String, i64>,
    pub snowflake_id_generator: Arc<SnowflakeIdGenerator>,
    pub redis_circuit_breaker: Arc<CircuitBreaker>,
    pub postgres_circuit_breaker: Arc<CircuitBreaker>,
}

impl AppState {
    pub async fn new(
        db_url: &str,
        hash_ring: Arc<ConsistentHashRing>,
        event_sender: Sender<ClickEvent>,
        moka_cache: Cache<String, i64>,
        snowflake_id_generator: Arc<SnowflakeIdGenerator>,
        redis_circuit_breaker: Arc<CircuitBreaker>,
        postgres_circuit_breaker: Arc<CircuitBreaker>,
    ) -> Self {
        let conn_pool = db::connection_pool(db_url).await;

        AppState {
            conn_pool,
            hash_ring,
            event_sender,
            moka_cache,
            snowflake_id_generator,
            redis_circuit_breaker,
            postgres_circuit_breaker,
        }
    }
}
