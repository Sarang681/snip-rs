mod encode;

use axum::Router;

#[tokio::main]
async fn main() {
    let router: Router<()> = Router::new();

    // bind to localhost port 8080 for now
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();

    axum::serve(listener, router).await.unwrap();
}
