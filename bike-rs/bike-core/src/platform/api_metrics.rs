//! Shared Prometheus metrics for Bike APIs.
//!
//! Call [`init_api_metrics`] once at startup with your app's namespace prefix
//! (e.g. `"mycorner_api"`, `"rss_api"`).  After that, the shared counters are
//! available via the accessor functions (`login_counter()`, etc.) and the axum
//! helpers (`metrics_middleware`, `metrics_route`) can be dropped into any app.
//!
//! For app-specific counters, call `registry()` after `init_api_metrics` and
//! register against the returned `Registry` so they appear on the same scrape
//! endpoint.
//!
//! ## Shared metrics
//! | Name                         | Type            | Labels                   |
//! |------------------------------|-----------------|--------------------------|
//! | `logins_total`               | Counter         | —                        |
//! | `failed_logins_total`        | Counter         | —                        |
//! | `logouts_total`              | Counter         | —                        |
//! | `token_refreshes_total`      | Counter         | —                        |
//! | `api_client_logins_total`    | Counter         | —                        |
//! | `app_errors_total`           | Counter         | —                        |
//! | `success_requests_total`     | Counter         | —                        |
//! | `tasks_enqueued_total`       | CounterVec      | type                     |
//! | `requests_total`             | CounterVec      | method, path, status     |
//! | `request_duration_seconds`   | HistogramVec    | method, path, status     |

use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::header::CONTENT_TYPE;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, Opts, Registry, TextEncoder,
};
use std::sync::OnceLock;
use std::time::Instant;

static REGISTRY: OnceLock<Registry> = OnceLock::new();
static LOGIN_COUNTER: OnceLock<IntCounter> = OnceLock::new();
static FAILED_LOGIN_COUNTER: OnceLock<IntCounter> = OnceLock::new();
static LOGOUT_COUNTER: OnceLock<IntCounter> = OnceLock::new();
static TOKEN_REFRESH_COUNTER: OnceLock<IntCounter> = OnceLock::new();
static API_CLIENT_LOGIN_COUNTER: OnceLock<IntCounter> = OnceLock::new();
static APP_ERRORS: OnceLock<IntCounter> = OnceLock::new();
static SUCCESS_REQUESTS: OnceLock<IntCounter> = OnceLock::new();
static TASKS_ENQUEUED: OnceLock<IntCounterVec> = OnceLock::new();
static REQUEST_COUNTER: OnceLock<IntCounterVec> = OnceLock::new();
static REQUEST_DURATION_SECONDS: OnceLock<HistogramVec> = OnceLock::new();

fn expect_init() -> &'static Registry {
    REGISTRY
        .get()
        .expect("Bike api_metrics not initialized; call init_api_metrics() at startup")
}

/// The shared metrics `Registry`.  Use this to register app-specific counters
/// after calling `init_api_metrics`.
pub fn registry() -> &'static Registry {
    expect_init()
}

pub fn login_counter() -> &'static IntCounter {
    LOGIN_COUNTER
        .get()
        .expect("Bike api_metrics not initialized")
}

pub fn failed_login_counter() -> &'static IntCounter {
    FAILED_LOGIN_COUNTER
        .get()
        .expect("Bike api_metrics not initialized")
}

pub fn logout_counter() -> &'static IntCounter {
    LOGOUT_COUNTER
        .get()
        .expect("Bike api_metrics not initialized")
}

pub fn token_refresh_counter() -> &'static IntCounter {
    TOKEN_REFRESH_COUNTER
        .get()
        .expect("Bike api_metrics not initialized")
}

pub fn api_client_login_counter() -> &'static IntCounter {
    API_CLIENT_LOGIN_COUNTER
        .get()
        .expect("Bike api_metrics not initialized")
}

pub fn tasks_enqueued() -> &'static IntCounterVec {
    TASKS_ENQUEUED
        .get()
        .expect("Bike api_metrics not initialized")
}

/// Initialize the shared API metrics registry with the given namespace prefix.
///
/// Must be called **once** at application startup before any metrics or
/// middleware are used.  Calling it a second time is a no-op.
pub fn init_api_metrics(namespace: &str) {
    // If already initialized, do nothing (safe for tests that call setup
    // multiple times in the same process).
    if REGISTRY.get().is_some() {
        return;
    }

    let registry = Registry::new_custom(Some(namespace.to_string()), None)
        .expect("failed to create api metrics registry");

    macro_rules! reg_counter {
        ($static:ident, $name:expr, $help:expr) => {{
            let c = IntCounter::new($name, $help)
                .unwrap_or_else(|e| panic!("failed to create {}: {}", $name, e));
            registry
                .register(Box::new(c.clone()))
                .unwrap_or_else(|e| panic!("failed to register {}: {}", $name, e));
            $static.set(c).ok();
        }};
    }

    macro_rules! reg_counter_vec {
        ($static:ident, $name:expr, $help:expr, $labels:expr) => {{
            let v = IntCounterVec::new(Opts::new($name, $help), $labels)
                .unwrap_or_else(|e| panic!("failed to create {}: {}", $name, e));
            registry
                .register(Box::new(v.clone()))
                .unwrap_or_else(|e| panic!("failed to register {}: {}", $name, e));
            $static.set(v).ok();
        }};
    }

    reg_counter!(
        LOGIN_COUNTER,
        "logins_total",
        "Number of successful user logins"
    );
    reg_counter!(
        FAILED_LOGIN_COUNTER,
        "failed_logins_total",
        "Number of failed login attempts"
    );
    reg_counter!(LOGOUT_COUNTER, "logouts_total", "Number of logouts");
    reg_counter!(
        TOKEN_REFRESH_COUNTER,
        "token_refreshes_total",
        "Number of token refreshes"
    );
    reg_counter!(
        API_CLIENT_LOGIN_COUNTER,
        "api_client_logins_total",
        "Number of successful API client logins"
    );
    reg_counter!(
        APP_ERRORS,
        "app_errors_total",
        "Number of application errors (5xx)"
    );
    reg_counter!(
        SUCCESS_REQUESTS,
        "success_requests_total",
        "Number of successful requests (2xx)"
    );

    reg_counter_vec!(
        TASKS_ENQUEUED,
        "tasks_enqueued_total",
        "Number of tasks enqueued for processing",
        &["type"]
    );
    reg_counter_vec!(
        REQUEST_COUNTER,
        "requests_total",
        "Total number of HTTP requests",
        &["method", "path", "status"]
    );

    register_request_duration(&registry);

    // Commit the registry before warmup so accessors work.
    REGISTRY.set(registry).ok();
    warmup_metrics();
}

fn register_request_duration(registry: &Registry) {
    let opts = HistogramOpts::new(
        "request_duration_seconds",
        "HTTP request latencies in seconds",
    )
    .buckets(vec![
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
    ]);
    let v = HistogramVec::new(opts, &["method", "path", "status"])
        .expect("failed to create request_duration_seconds");
    registry
        .register(Box::new(v.clone()))
        .expect("failed to register request_duration_seconds");
    REQUEST_DURATION_SECONDS.set(v).ok();
}

fn warmup_metrics() {
    // Warmup: touch zero values so metrics appear in the first scrape.
    login_counter().inc_by(0);
    failed_login_counter().inc_by(0);
    logout_counter().inc_by(0);
    token_refresh_counter().inc_by(0);
    api_client_login_counter().inc_by(0);
    APP_ERRORS.get().unwrap().inc_by(0);
    SUCCESS_REQUESTS.get().unwrap().inc_by(0);
    REQUEST_COUNTER
        .get()
        .unwrap()
        .with_label_values(&["GET", "/health", "200"])
        .inc_by(0);
    REQUEST_DURATION_SECONDS
        .get()
        .unwrap()
        .with_label_values(&["GET", "/health", "200"])
        .observe(0.0);
}

/// Axum middleware that records HTTP request counts and latencies.
///
/// Add via `.layer(axum::middleware::from_fn(bike_core::platform::api_metrics::metrics_middleware))`.
pub async fn metrics_middleware(req: Request<Body>, next: Next) -> Response {
    let method = req.method().to_string();
    let raw_path = req.uri().path().to_string();

    // Don't instrument the scrape endpoint itself.
    if raw_path == "/metrics" {
        return next.run(req).await;
    }

    // Prefer the matched route template to avoid high cardinality.
    let mut path_label = if let Some(matched) = req.extensions().get::<MatchedPath>() {
        matched.as_str().to_string()
    } else {
        raw_path.clone()
    };

    // Collapse all upload paths to a single label.
    let is_uploads = path_label.starts_with("/uploads") || raw_path.starts_with("/uploads");
    if is_uploads {
        path_label = "/uploads".to_string();
    }

    let start = Instant::now();
    let resp = next.run(req).await;
    let elapsed = start.elapsed().as_secs_f64();
    let status = resp.status().as_u16().to_string();

    if resp.status().is_success() {
        SUCCESS_REQUESTS.get().unwrap().inc();
    } else if resp.status().is_server_error() {
        APP_ERRORS.get().unwrap().inc();
    }

    REQUEST_COUNTER
        .get()
        .unwrap()
        .with_label_values(&[&method, &path_label, &status])
        .inc();

    // Skip histogram for upload binary data (noisy artifacts).
    if !is_uploads {
        REQUEST_DURATION_SECONDS
            .get()
            .unwrap()
            .with_label_values(&[&method, &path_label, &status])
            .observe(elapsed);
    }

    resp
}

/// Axum handler that renders the Prometheus metrics scrape page.
///
/// Mount at `"/metrics"`:
/// ```ignore
/// .route("/metrics", axum::routing::get(bike_core::platform::api_metrics::metrics_route))
/// ```
pub async fn metrics_route() -> Response {
    let encoder = TextEncoder::new();
    let metric_families = expect_init().gather();
    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .unwrap_or_default();
    let body = String::from_utf8(buffer).unwrap_or_default();
    Response::builder()
        .header(CONTENT_TYPE, encoder.format_type())
        .body(Body::from(body))
        .unwrap()
}
