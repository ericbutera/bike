pub mod app_error;
pub mod activity_details;
pub mod activity_lifecycle;
pub mod activity_location;
pub mod activity_summary;
pub mod config;
pub mod controllers;
pub mod entities;
pub mod feature_flags_keys;
pub mod metrics;
pub mod openapi;
pub mod segment_support;
pub mod storage;
pub mod tasks;

use crate::config::Config;
use crate::openapi::ApiDoc;
use crate::storage::AppStorage;
use axum::http::{HeaderName, HeaderValue, Method};
use axum::middleware::from_fn;
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub async fn app(app_state: Arc<AppStorage>) -> Router {
    let cfg = Config::get();
    let origins: Vec<HeaderValue> = cfg
        .cors_allowed_origins
        .iter()
        .filter_map(|o| HeaderValue::from_str(o).ok())
        .collect();

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(vec![
            HeaderName::from_static("authorization"),
            HeaderName::from_static("content-type"),
            HeaderName::from_static("accept"),
            HeaderName::from_static("origin"),
            HeaderName::from_static("x-requested-with"),
        ])
        .allow_credentials(true);

    let openapi = ApiDoc::openapi();

    controllers::routes()
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", openapi))
        .route("/metrics", get(metrics::metrics_route))
        .layer(cors)
        .layer(from_fn(metrics::metrics_middleware))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state)
}

pub async fn init_tracing_subscriber() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}
