pub mod activity_import_history;
pub mod activity_import_lock;
pub mod activity_location;
pub mod app_error;
pub mod controllers;
pub mod feature_flags_keys;
pub mod metrics;
pub mod openapi;
pub mod segment_support;
pub mod storage;
pub mod tasks;
pub mod xc_goal_backfill;

use crate::openapi::ApiDoc;
use crate::storage::AppStorage;
use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request};
use axum::middleware::{from_fn, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use bike_core::config::Config;
use bike_core::observability;
use std::sync::Arc;
use std::time::Duration;
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
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(make_http_trace_span)
                .on_response(record_http_response),
        )
        .with_state(app_state)
}

pub fn init_tracing_subscriber() -> observability::ObservabilityGuard {
    observability::init_observability("bike-rust-api")
}

fn make_http_trace_span<B>(request: &Request<B>) -> tracing::Span {
    let span = if should_trace_http_path(request.uri().path()) {
        let request_id = request_id_from_headers(request.headers());
        tracing::info_span!(
            target: "api",
            "request",
            "otel.kind" = "server",
            request_id = request_id,
            "http.request.header.x_request_id" = request_id,
            method = %request.method(),
            "http.request.method" = %request.method(),
            "http.route" = matched_route(request),
            "http.response.status_code" = tracing::field::Empty,
            "otel.status_code" = tracing::field::Empty,
            "otel.status_description" = tracing::field::Empty,
            version = ?request.version(),
        )
    } else {
        tracing::Span::none()
    };

    observability::set_span_parent_from_headers(&span, request.headers());
    observability::record_span_trace_context(&span);
    span
}

fn matched_route<B>(request: &Request<B>) -> &str {
    request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or("unmatched")
}

fn record_http_response(response: &Response<Body>, _latency: Duration, span: &tracing::Span) {
    let status = response.status();
    span.record("http.response.status_code", i64::from(status.as_u16()));

    if status.is_server_error() {
        span.record("otel.status_code", "ERROR");
        span.record(
            "otel.status_description",
            status.canonical_reason().unwrap_or("server error"),
        );
    }
}

fn should_trace_http_path(path: &str) -> bool {
    !matches!(
        path,
        "/metrics"
            | "/api/health"
            | "/api/ready"
            | "/health"
            | "/healthz"
            | "/ready"
            | "/readyz"
            | "/openapi.json"
    ) && !path.starts_with("/swagger-ui/")
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

#[cfg(test)]
mod tracing_filter_tests {
    use super::{make_http_trace_span, record_http_response, should_trace_http_path};
    use axum::body::Body;
    use axum::http::{Request, Response, StatusCode};
    use opentelemetry::trace::{Status, TracerProvider as _};
    use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
    use std::time::Duration;
    use tower::ServiceExt;
    use tower_http::trace::TraceLayer;
    use tracing_subscriber::prelude::*;

    #[test]
    fn filters_health_scrape_and_static_documentation_paths() {
        for path in [
            "/metrics",
            "/api/health",
            "/api/ready",
            "/healthz",
            "/readyz",
            "/openapi.json",
            "/swagger-ui/index.html",
        ] {
            assert!(!should_trace_http_path(path), "unexpected trace for {path}");
        }
    }

    #[test]
    fn retains_application_routes() {
        assert!(should_trace_http_path("/api/segments"));
    }

    #[test]
    fn marks_server_failures_as_otel_errors() {
        let (exporter, spans) = export_response_span(StatusCode::INTERNAL_SERVER_ERROR);

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].status, Status::error("Internal Server Error"));
        assert!(spans[0].attributes.iter().any(|attribute| {
            attribute.key.as_str() == "http.response.status_code"
                && attribute.value == 500_i64.into()
        }));
        drop(exporter);
    }

    #[test]
    fn leaves_client_errors_unset_for_tail_sampling_semantics() {
        let (_exporter, spans) = export_response_span(StatusCode::UNAUTHORIZED);

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].status, Status::Unset);
        assert!(spans[0].attributes.iter().any(|attribute| {
            attribute.key.as_str() == "http.response.status_code"
                && attribute.value == 401_i64.into()
        }));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn records_templated_route_without_raw_url_path() {
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("api-route-test")));
        let app = axum::Router::new()
            .route(
                "/api/segments/:id/effort-analysis",
                axum::routing::get(|| async { StatusCode::OK }),
            )
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(make_http_trace_span)
                    .on_response(record_http_response),
            );
        let request = Request::builder()
            .uri("/api/segments/123456/effort-analysis?split_count=10")
            .body(Body::empty())
            .expect("build test request");

        let _subscriber = tracing::subscriber::set_default(subscriber);
        let response = app.oneshot(request).await.expect("run test request");
        assert_eq!(response.status(), StatusCode::OK);
        drop(response);
        drop(_subscriber);

        let spans = exporter.get_finished_spans().expect("export route span");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].attributes.iter().any(|attribute| {
            attribute.key.as_str() == "http.route"
                && attribute.value == "/api/segments/:id/effort-analysis".into()
        }));
        assert!(!spans[0]
            .attributes
            .iter()
            .any(|attribute| attribute.key.as_str() == "url.path"));
    }

    fn export_response_span(
        status: StatusCode,
    ) -> (
        InMemorySpanExporter,
        Vec<opentelemetry_sdk::trace::SpanData>,
    ) {
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("api-status-test")));

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!(
                "request",
                "otel.kind" = "server",
                "http.response.status_code" = tracing::field::Empty,
                "otel.status_code" = tracing::field::Empty,
                "otel.status_description" = tracing::field::Empty,
            );
            let response = Response::builder()
                .status(status)
                .body(Body::empty())
                .expect("build test response");
            record_http_response(&response, Duration::from_millis(1), &span);
            drop(span);
        });

        let spans = exporter.get_finished_spans().expect("export response span");
        (exporter, spans)
    }
}
