use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

pub struct FetchedLink {
    pub long_url: String,
    pub expiration_date: Option<OffsetDateTime>,
}

#[derive(Deserialize, Debug, ToSchema)]
pub struct ShortenRequest {
    /// The original, long URL to be shortened
    #[schema(required = true, value_type = String, format = "uri", example="https://example.com/very/long/url")]
    pub long_url: String,
    ///Optional Unix timestamp in **seconds** when the link should expire.
    ///If omitted, the link will never expire.
    #[schema(required = false, value_type = i64, format = "int64", example = 1781049600)]
    pub expiration_date: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct ShortenResponse {
    ///The generated Base62 short code.
    #[schema(value_type = String, example = "3Jt")]
    pub short_code: String,
}

#[derive(Debug)]
pub struct ClickEvent {
    pub short_code: String,
    pub ip_addr: String,
    pub referrer: String,
    pub user_agent: String,
    pub clicked_at: OffsetDateTime,
}
