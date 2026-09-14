// Shared auth + HTTP metrics live in glass::api_metrics.
// This module is a thin wrapper that initialises the shared registry with the
// app namespace supplied by the generated project.

use crate::entities::{provider_rate_limit_buckets, strava_connections};
use crate::storage::AppStorage;
use axum::body::Body;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::response::Response;
use once_cell::sync::Lazy;
use prometheus::{
    register_int_counter_vec, register_int_gauge, register_int_gauge_vec, Encoder, IntCounterVec,
    IntGauge, IntGaugeVec, TextEncoder,
};
use sea_orm::{DbErr, EntityTrait, PaginatorTrait};
use std::sync::Arc;

pub use kaleido::glass::api_metrics::metrics_middleware;

static PROVIDER_API_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "bike_provider_api_requests_total",
        "Total outbound provider API requests by provider, operation, request class, and status.",
        &["provider", "operation", "request_class", "status"]
    )
    .expect("register provider API request counter")
});

static PROVIDER_API_REQUESTS_15_MINUTES: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "bike_provider_api_requests_15_minutes",
        "Current outbound provider API request count in the active 15-minute window.",
        &["provider", "request_class"]
    )
    .expect("register provider 15-minute API request gauge")
});

static PROVIDER_API_REQUESTS_DAILY: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "bike_provider_api_requests_daily",
        "Current outbound provider API request count in the active daily window.",
        &["provider", "request_class"]
    )
    .expect("register provider daily API request gauge")
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

static STRAVA_CONNECTED_ATHLETES: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "bike_strava_connected_athletes",
        "Current number of Strava athletes connected to Bike."
    )
    .expect("register Strava connected athletes gauge")
});

/// Initialize all API metrics.  Must be called once at startup.
pub fn init_metrics() {
    kaleido::glass::api_metrics::init_api_metrics("bike_api");
    Lazy::force(&PROVIDER_API_REQUESTS_TOTAL);
    Lazy::force(&PROVIDER_API_REQUESTS_15_MINUTES);
    Lazy::force(&PROVIDER_API_REQUESTS_DAILY);
    Lazy::force(&PROVIDER_RATE_LIMIT_PAUSES_TOTAL);
    Lazy::force(&PROVIDER_RATE_LIMIT_LIMIT);
    Lazy::force(&PROVIDER_RATE_LIMIT_USED);
    Lazy::force(&PROVIDER_RATE_LIMIT_REMAINING);
    Lazy::force(&PROVIDER_RATE_LIMIT_RESET_TIMESTAMP);
    Lazy::force(&STRAVA_CONNECTED_ATHLETES);
}

pub async fn metrics_route(State(state): State<Arc<AppStorage>>) -> Response {
    if let Err(error) = refresh_database_metrics(&state).await {
        tracing::warn!(error = ?error, "failed to refresh database-backed metrics");
    }

    let encoder = TextEncoder::new();
    let mut metric_families = kaleido::glass::api_metrics::registry().gather();
    metric_families.extend(prometheus::gather());

    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .unwrap_or_default();
    let body = String::from_utf8(buffer).unwrap_or_default();

    Response::builder()
        .header(CONTENT_TYPE, encoder.format_type())
        .body(Body::from(body))
        .expect("failed to build metrics response")
}

async fn refresh_database_metrics(state: &AppStorage) -> Result<(), DbErr> {
    let connected_athletes = strava_connections::Entity::find()
        .count(&state.db)
        .await?
        .min(i64::MAX as u64) as i64;

    STRAVA_CONNECTED_ATHLETES.set(connected_athletes);

    let provider_rate_limit_rows = provider_rate_limit_buckets::Entity::find()
        .all(&state.db)
        .await?;

    for row in provider_rate_limit_rows {
        set_provider_api_request_window_count(
            &row.provider,
            &row.bucket,
            row.used_count,
            row.reset_at.timestamp(),
        );
    }

    Ok(())
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

    set_provider_api_request_window_count(provider, bucket, effective_used, reset_timestamp);
}

fn set_provider_api_request_window_count(
    provider: &str,
    bucket: &str,
    used_count: i32,
    reset_timestamp: i64,
) {
    let Some((window, request_class)) = provider_api_request_window_labels(bucket) else {
        return;
    };

    let active_used_count = if reset_timestamp <= chrono::Utc::now().timestamp() {
        0
    } else {
        i64::from(used_count.max(0))
    };

    match window {
        ProviderApiRequestWindow::FifteenMinutes => PROVIDER_API_REQUESTS_15_MINUTES
            .with_label_values(&[provider, request_class])
            .set(active_used_count),
        ProviderApiRequestWindow::Daily => PROVIDER_API_REQUESTS_DAILY
            .with_label_values(&[provider, request_class])
            .set(active_used_count),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderApiRequestWindow {
    FifteenMinutes,
    Daily,
}

fn provider_api_request_window_labels(
    bucket: &str,
) -> Option<(ProviderApiRequestWindow, &'static str)> {
    match bucket {
        "overall_15_minute" => Some((ProviderApiRequestWindow::FifteenMinutes, "overall")),
        "read_15_minute" => Some((ProviderApiRequestWindow::FifteenMinutes, "read")),
        "overall_daily" => Some((ProviderApiRequestWindow::Daily, "overall")),
        "read_daily" => Some((ProviderApiRequestWindow::Daily, "read")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{provider_api_request_window_labels, ProviderApiRequestWindow};

    #[test]
    fn maps_provider_rate_limit_buckets_to_request_window_metrics() {
        assert_eq!(
            provider_api_request_window_labels("overall_15_minute"),
            Some((ProviderApiRequestWindow::FifteenMinutes, "overall"))
        );
        assert_eq!(
            provider_api_request_window_labels("read_15_minute"),
            Some((ProviderApiRequestWindow::FifteenMinutes, "read"))
        );
        assert_eq!(
            provider_api_request_window_labels("overall_daily"),
            Some((ProviderApiRequestWindow::Daily, "overall"))
        );
        assert_eq!(
            provider_api_request_window_labels("read_daily"),
            Some((ProviderApiRequestWindow::Daily, "read"))
        );
        assert_eq!(provider_api_request_window_labels("remote_429"), None);
    }
}
