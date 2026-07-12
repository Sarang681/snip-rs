use crate::snowflake::SnowflakeIdGenerator;
use crate::{models::ClickEvent, state::AppState};
use moka::future::Cache;
use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};
use tokio::sync::mpsc;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

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
    let redis_url = std::env::var("REDIS_URL").expect("Redis URL not provided");
    let machine_id: u16 = std::env::var("MACHINE_ID")
        .as_deref()
        .unwrap_or("1")
        .parse::<u16>()
        .expect("Invalid machine id");
    if machine_id > 1023 {
        panic!("Machine ID must be between 0 and 1023");
    }
    let moka_cache: Cache<String, i64> = Cache::builder()
        .max_capacity(50_000)
        .time_to_live(Duration::from_secs(60))
        .build();
    let snowflake_id_generator = Arc::new(SnowflakeIdGenerator::new(machine_id));

    let (tx, rx) = mpsc::channel::<ClickEvent>(100_000);

    let app_state = AppState::new(
        &db_url,
        &redis_url,
        tx.clone(),
        moka_cache,
        snowflake_id_generator,
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
