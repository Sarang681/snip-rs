use crate::state::AppState;

mod db;
mod encode;
mod error;
mod routes;
mod state;

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
