use axum::Router;

use crate::AppState;

mod urls;

pub fn app_router() -> Router<AppState> {
    Router::new().merge(urls::router())
}
