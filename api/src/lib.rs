pub mod activity_analytics;
pub mod activity_details;
pub mod activity_training_analysis;
pub mod activity_import_lock;
pub mod activity_import_pipeline;
pub mod activity_lifecycle;
pub mod activity_location;
pub mod activity_summary;
pub mod analytics;
pub mod app_error;
pub mod archive_import;
pub mod config;
pub mod controllers;
pub mod dedupe;
pub mod entities;
pub mod feature_flags_keys;
pub mod fit_support;
pub mod integration_events;
pub mod metrics;
pub mod openapi;
pub mod segment_support;
pub mod storage;
pub mod strava;
pub mod tasks;
pub mod training_profile;
pub mod xc_goal_backfill;

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

    if let Err(error) = crate::strava::ensure_webhook_subscription_registered(&app_state.db).await {
        tracing::warn!(
            message = %error.message,
            "failed to ensure Strava webhook subscription during API startup"
        );
    }

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
