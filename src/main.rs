use std::{net::SocketAddr, time::Duration};

use crate::{models::ClickEvent, state::AppState};
use moka::future::Cache;
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
    let moka_cache: Cache<String, i64> = Cache::builder()
        .max_capacity(50_000)
        .time_to_live(Duration::from_secs(60))
        .build();

    let (tx, rx) = mpsc::channel::<ClickEvent>(100_000);

    let app_state = AppState::new(&db_url, &redis_url, tx.clone(), moka_cache).await;

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
