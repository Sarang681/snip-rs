use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

pub async fn connection_pool(url: &str) -> Pool<Postgres> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("Failed to create DB connection pool")
}
