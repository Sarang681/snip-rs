use crate::state::AppState;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod db;
mod encode;
mod error;
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

    let app_state = AppState::new(&db_url, &redis_url).await;

    let app = routes::app_router()
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    // bind to localhost port 8080 for now
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}
