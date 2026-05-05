use axum::Router;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new();
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
