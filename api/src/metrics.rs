// Shared auth + HTTP metrics live in glass::api_metrics.
// This module is a thin wrapper that initialises the shared registry with the
// app namespace supplied by the generated project.

use crate::entities::strava_connections;
use crate::storage::AppStorage;
use axum::body::Body;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::response::Response;
use once_cell::sync::Lazy;
use prometheus::{register_int_gauge, Encoder, IntGauge, TextEncoder};
use sea_orm::{DbErr, EntityTrait, PaginatorTrait};
use std::sync::Arc;

pub use bike_core::provider_metrics::{
    init_provider_metrics, record_provider_api_request, record_provider_rate_limit_pause,
    set_provider_rate_limit_bucket,
};
pub use kaleido::glass::api_metrics::metrics_middleware;

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
    init_provider_metrics();
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

    Ok(())
}
