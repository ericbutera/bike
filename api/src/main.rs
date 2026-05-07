use api::config::Config;
use api::storage::AppStorage;
use api::{app, init_tracing_subscriber, metrics};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    init_tracing_subscriber().await;
    metrics::init_metrics();
    let cfg = Config::init_from_env();

    let storage = AppStorage::new(&cfg.database_url).await;
    let app_state = Arc::new(storage);
    let app = app(app_state).await;

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind API listener");

    tracing::info!("api listening on http://{}", addr);
    axum::serve(listener, app)
        .await
        .expect("Failed to start API server");
}
