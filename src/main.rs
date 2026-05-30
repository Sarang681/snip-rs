use sqlx::{Pool, Postgres};

mod db;
mod encode;
mod routes;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let db_url = std::env::var("DATABASE_URL").expect("Databse URL not provided");
    let app_state = AppState::new(&db_url).await;

    let app = routes::app_router().with_state(app_state);

    // bind to localhost port 8080 for now
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

#[derive(Clone)]
struct AppState {
    conn_pool: Pool<Postgres>,
}

impl AppState {
    async fn new(url: &str) -> Self {
        let conn_pool = db::connection_pool(url).await;
        AppState { conn_pool }
    }
}
