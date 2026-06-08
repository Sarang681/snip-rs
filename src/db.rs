use sqlx::{Pool, Postgres, postgres::PgPoolOptions, types::time::OffsetDateTime};

use crate::error::AppError;

pub struct FetchedLink {
    pub id: u64,
    pub long_url: String,
    pub expiration_date: Option<OffsetDateTime>,
}

pub async fn connection_pool(url: &str) -> Pool<Postgres> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("Failed to create DB connection pool")
}

pub async fn add_url(
    url: &str,
    expiration_date: OffsetDateTime,
    pool: &Pool<Postgres>,
) -> Result<u64, AppError> {
    let result = sqlx::query!(
        r#"
    INSERT INTO urls (long_url, expiration_date)
        VALUES ($1, $2)
        RETURNING id
    "#,
        url,
        expiration_date
    )
    .fetch_one(pool)
    .await?;

    Ok(result.id as u64)
}

pub async fn fetch_url(id: u64, pool: &Pool<Postgres>) -> Result<FetchedLink, AppError> {
    let result = sqlx::query!(
        r#"
    SELECT id, long_url, expiration_date
    FROM urls
    WHERE id = ($1)
    "#,
        id as i64
    )
    .fetch_one(pool)
    .await?;

    Ok(FetchedLink {
        id: result.id as u64,
        long_url: result.long_url,
        expiration_date: result.expiration_date,
    })
}
