use sqlx::{Pool, Postgres};
use tokio::sync::mpsc::Receiver;

use crate::{db, models::ClickEvent};
pub async fn receive(mut receiver: Receiver<ClickEvent>, conn_pool: Pool<Postgres>) {
    let mut batch: Vec<ClickEvent> = Vec::with_capacity(500);
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                if !batch.is_empty() {
                    //flush all the events to db
                    if let Err(_) = db::add_event_click(&batch, &conn_pool).await {
                    tracing::warn!("Error inserting clicks into db");
                }
                    batch.clear();
                }
            }

            Some(event) = receiver.recv() => {
                batch.push(event);
                if batch.len() >= 500 {
                    //flush all the events to db
                    if let Err(_) = db::add_event_click(&batch, &conn_pool).await {
                    tracing::warn!("Error inserting clicks into db");
                }
                    batch.clear();
                }
            }

            else => {
                if !batch.is_empty() {
                    //flush all the events to db
                    if let Err(_) = db::add_event_click(&batch, &conn_pool).await {
                    tracing::warn!("Error inserting clicks into db");
                }
                    batch.clear();
                }
                break;
            }
        }
    }
}
