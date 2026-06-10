#[derive(OpenApi)]
#[openapi(paths(handle_redirect_from_short_code, handle_shorten_url))]
pub struct ApiDoc;
