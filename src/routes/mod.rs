use axum::Router;

use crate::AppState;

pub mod urls;

pub fn app_router() -> Router<AppState> {
    Router::new().merge(urls::router())
}
