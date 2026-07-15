use crate::circuit_breaker::CircuitBreaker;
use crate::consistent_hashing::ConsistentHashRing;
use crate::snowflake::SnowflakeIdGenerator;
use crate::{models::ClickEvent, state::AppState};
use moka::future::Cache;
use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};
use tokio::sync::mpsc;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod circuit_breaker;
mod consistent_hashing;
mod db;
mod encode;
mod error;
mod models;
mod openapi;
mod ratelimit;
mod receiver;
mod redis;
mod routes;
mod snowflake;
mod state;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_url = std::env::var("DATABASE_URL").expect("Databse URL not provided");
    let redis_nodes =
        std::env::var("REDIS_NODES").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let machine_id: u16 = std::env::var("MACHINE_ID")
        .as_deref()
        .unwrap_or("1")
        .parse::<u16>()
        .expect("Invalid machine id");
    if machine_id > 1023 {
        panic!("Machine ID must be between 0 and 1023");
    }

    // Create the hash ring (150 virtual nodes per physical node)
    let mut hash_ring = ConsistentHashRing::new(150);
    // Add each Redis node to the ring
    for node_url in redis_nodes.split(',') {
        let node_url = node_url.trim();

        match redis::get_redis_client(node_url).await {
            Ok(client) => {
                let node_id = node_url.to_string();
                tracing::info!("Adding Redis node: {}", node_id);
                hash_ring.add_node(client, &node_id);
            }
            Err(e) => {
                tracing::warn!("Failed to connect to Redis node {}: {}", node_url, e);
                // Skip this node, don't add it to the ring
            }
        }
    }

    info!(
        "Hash ring has {} virtual nodes",
        hash_ring.node_mappings.len()
    );
    for (pos, node_id) in hash_ring.node_mappings.iter() {
        info!("Position {}: {}", pos, node_id);
    }
    let hash_ring = Arc::new(hash_ring);
    let moka_cache: Cache<String, i64> = Cache::builder()
        .max_capacity(50_000)
        .time_to_live(Duration::from_secs(60))
        .build();
    let snowflake_id_generator = Arc::new(SnowflakeIdGenerator::new(machine_id));

    let (tx, rx) = mpsc::channel::<ClickEvent>(100_000);

    let redis_circuit_breaker = Arc::new(CircuitBreaker::new(5, 10_000, 10));
    let postgres_circuit_breaker = Arc::new(CircuitBreaker::new(5, 10_000, 10));

    let app_state = AppState::new(
        &db_url,
        hash_ring,
        tx.clone(),
        moka_cache,
        snowflake_id_generator,
        redis_circuit_breaker,
        postgres_circuit_breaker,
    )
    .await;

    drop(tx);

    let conn_pool = app_state.conn_pool.clone();

    tokio::spawn(async move { receiver::receive(rx, conn_pool).await });

    let app = routes::app_router()
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    // bind to localhost port 8080 for now
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
