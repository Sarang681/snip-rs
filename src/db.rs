use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

use crate::error::AppError;

pub async fn connection_pool(url: &str) -> Pool<Postgres> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("Failed to create DB connection pool")
}

pub async fn add_url(url: &str, pool: &Pool<Postgres>) -> Result<u64, AppError> {
    let result = sqlx::query!(
        r#"
    INSERT INTO urls (long_url)
        VALUES ($1)
        RETURNING id
    "#,
        url
    )
    .fetch_one(pool)
    .await?;

    Ok(result.id as u64)
}

pub async fn fetch_url(id: u64, pool: &Pool<Postgres>) -> Result<String, AppError> {
    let result = sqlx::query!(
        r#"
    SELECT long_url
    FROM urls
    WHERE id = ($1)
    "#,
        id as i64
    )
    .fetch_one(pool)
    .await?;

    Ok(result.long_url)
}
