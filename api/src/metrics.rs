// Shared auth + HTTP metrics live in glass::api_metrics.
// This module is a thin wrapper that initialises the shared registry with the
// app namespace supplied by the generated project.

use once_cell::sync::Lazy;
use prometheus::{register_int_counter_vec, register_int_gauge_vec, IntCounterVec, IntGaugeVec};

pub use kaleido::glass::api_metrics::{metrics_middleware, metrics_route};

static PROVIDER_API_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "bike_provider_api_requests_total",
        "Total outbound provider API requests by provider, operation, request class, and status.",
        &["provider", "operation", "request_class", "status"]
    )
    .expect("register provider API request counter")
});

static PROVIDER_RATE_LIMIT_PAUSES_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "bike_provider_rate_limit_pauses_total",
        "Total outbound provider API pauses caused by local or remote provider rate limits.",
        &["provider", "bucket", "operation"]
    )
    .expect("register provider rate-limit pause counter")
});

static PROVIDER_RATE_LIMIT_LIMIT: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "bike_provider_rate_limit_limit",
        "Current provider API quota limit by provider and bucket.",
        &["provider", "bucket"]
    )
    .expect("register provider rate-limit limit gauge")
});

static PROVIDER_RATE_LIMIT_USED: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "bike_provider_rate_limit_used",
        "Current provider API quota used count by provider and bucket.",
        &["provider", "bucket"]
    )
    .expect("register provider rate-limit used gauge")
});

static PROVIDER_RATE_LIMIT_REMAINING: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "bike_provider_rate_limit_remaining",
        "Current provider API quota remaining count by provider and bucket.",
        &["provider", "bucket"]
    )
    .expect("register provider rate-limit remaining gauge")
});

static PROVIDER_RATE_LIMIT_RESET_TIMESTAMP: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "bike_provider_rate_limit_reset_timestamp_seconds",
        "Provider API quota reset time as a Unix timestamp by provider and bucket.",
        &["provider", "bucket"]
    )
    .expect("register provider rate-limit reset timestamp gauge")
});

/// Initialize all API metrics.  Must be called once at startup.
pub fn init_metrics() {
    kaleido::glass::api_metrics::init_api_metrics("bike_api");
    Lazy::force(&PROVIDER_API_REQUESTS_TOTAL);
    Lazy::force(&PROVIDER_RATE_LIMIT_PAUSES_TOTAL);
    Lazy::force(&PROVIDER_RATE_LIMIT_LIMIT);
    Lazy::force(&PROVIDER_RATE_LIMIT_USED);
    Lazy::force(&PROVIDER_RATE_LIMIT_REMAINING);
    Lazy::force(&PROVIDER_RATE_LIMIT_RESET_TIMESTAMP);
}

pub fn record_provider_api_request(
    provider: &str,
    operation: &str,
    request_class: &str,
    status: &str,
) {
    PROVIDER_API_REQUESTS_TOTAL
        .with_label_values(&[provider, operation, request_class, status])
        .inc();
}

pub fn record_provider_rate_limit_pause(provider: &str, bucket: &str, operation: &str) {
    PROVIDER_RATE_LIMIT_PAUSES_TOTAL
        .with_label_values(&[provider, bucket, operation])
        .inc();
}

pub fn set_provider_rate_limit_bucket(
    provider: &str,
    bucket: &str,
    limit_count: i32,
    used_count: i32,
    reset_timestamp: i64,
) {
    let effective_limit = limit_count.max(0);
    let effective_used = used_count.clamp(0, effective_limit);
    let remaining = effective_limit.saturating_sub(effective_used);
    let labels = &[provider, bucket];

    PROVIDER_RATE_LIMIT_LIMIT
        .with_label_values(labels)
        .set(i64::from(effective_limit));
    PROVIDER_RATE_LIMIT_USED
        .with_label_values(labels)
        .set(i64::from(effective_used));
    PROVIDER_RATE_LIMIT_REMAINING
        .with_label_values(labels)
        .set(i64::from(remaining));
    PROVIDER_RATE_LIMIT_RESET_TIMESTAMP
        .with_label_values(labels)
        .set(reset_timestamp);
}
