use sqlx::{Pool, Postgres};

use crate::db;

#[derive(Clone)]
pub struct AppState {
    pub conn_pool: Pool<Postgres>,
}

impl AppState {
    pub async fn new(url: &str) -> Self {
        let conn_pool = db::connection_pool(url).await;
        AppState { conn_pool }
    }
}
