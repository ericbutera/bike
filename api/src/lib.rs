pub mod activity_analytics;
pub mod activity_details;
pub mod activity_import_lock;
pub mod activity_import_pipeline;
pub mod activity_lifecycle;
pub mod activity_location;
pub mod activity_parser;
pub mod activity_summary;
pub mod activity_training_analysis;
pub mod activity_type;
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
pub mod observability;
pub mod openapi;
pub mod provider_rate_limit;
pub mod segment_support;
pub mod storage;
pub mod strava;
pub mod strava_client;
pub mod strava_provider_payload;
pub mod tasks;
pub mod training_profile;
pub mod xc_goal_backfill;

use crate::config::Config;
use crate::openapi::ApiDoc;
use crate::storage::AppStorage;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request};
use axum::middleware::{from_fn, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

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
            HeaderName::from_static("baggage"),
            HeaderName::from_static("origin"),
            HeaderName::from_static("traceparent"),
            HeaderName::from_static("tracestate"),
            REQUEST_ID_HEADER,
            HeaderName::from_static("x-requested-with"),
        ])
        .allow_credentials(true);

    let openapi = ApiDoc::openapi();

    controllers::routes()
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", openapi))
        .route("/metrics", get(metrics::metrics_route))
        .layer(cors)
        .layer(from_fn(metrics::metrics_middleware))
        .layer(from_fn(request_id_response_header))
        .layer(TraceLayer::new_for_http().make_span_with(make_http_trace_span))
        .with_state(app_state)
}

pub fn init_tracing_subscriber() -> observability::ObservabilityGuard {
    observability::init_observability("bike-api")
}

fn make_http_trace_span<B>(request: &Request<B>) -> tracing::Span {
    let span = if request.uri().path() == "/metrics" {
        tracing::Span::none()
    } else {
        let request_id = request_id_from_headers(request.headers());
        tracing::info_span!(
            target: "api",
            "request",
            "otel.kind" = "server",
            request_id = request_id,
            "http.request.header.x_request_id" = request_id,
            method = %request.method(),
            uri = %request.uri(),
            "http.request.method" = %request.method(),
            "url.path" = request.uri().path(),
            version = ?request.version(),
        )
    };

    observability::set_span_parent_from_headers(&span, request.headers());
    observability::record_span_trace_context(&span);
    span
}

async fn request_id_response_header(request: Request<Body>, next: Next) -> Response {
    let request_id = request.headers().get(&REQUEST_ID_HEADER).cloned();
    let mut response = next.run(request).await;

    if let Some(request_id) = request_id {
        response
            .headers_mut()
            .insert(&REQUEST_ID_HEADER, request_id);
    }

    response
}

fn request_id_from_headers(headers: &HeaderMap) -> &str {
    headers
        .get(&REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
}
