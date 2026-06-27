use sqlx::{Pool, Postgres, QueryBuilder, postgres::PgPoolOptions, types::time::OffsetDateTime};

use crate::{
    error::AppError,
    models::{ClickEvent, FetchedLink},
};
pub async fn connection_pool(url: &str) -> Pool<Postgres> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(url)
        .await
        .expect("Failed to create DB connection pool")
}

pub async fn add_url(
    url: &str,
    expiration_date: Option<OffsetDateTime>,
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
        long_url: result.long_url,
        expiration_date: result.expiration_date,
    })
}

pub async fn add_event_click(
    clicks: &Vec<ClickEvent>,
    pool: &Pool<Postgres>,
) -> Result<(), AppError> {
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "INSERT INTO clicks (short_code, ip_addr, referrer, user_agent, clicked_at)",
    );

    query_builder.push_values(clicks, |mut b, click| {
        b.push_bind(&click.short_code)
            .push_bind(&click.ip_addr)
            .push_bind(&click.referrer)
            .push_bind(&click.user_agent)
            .push_bind(click.clicked_at);
    });

    let query = query_builder.build();
    query.execute(pool).await?;
    Ok(())
}
