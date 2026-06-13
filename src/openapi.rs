use utoipa::OpenApi;

use crate::routes::urls;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "snip-rs API",
        description = " A high-performance URL shortener API built with Rust, Axum, PostgreSQL, and Redis.
    
    This API allows you to shorten long URLs into compact Base62 codes and provides 
    intelligent redirect routing with optional link expiration and click tracking.",
        version = "1.0.0"
    ),
    paths(urls::handle_redirect_from_short_code, urls::handle_shorten_url)
)]
pub struct ApiDoc;
