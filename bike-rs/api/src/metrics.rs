// Shared auth + HTTP metrics live in glass::api_metrics.
// This module is a thin wrapper that initialises the shared registry with the
// app namespace supplied by the generated project.

use crate::storage::AppStorage;
use axum::body::Body;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::response::Response;
use bike_core::entities::{activity_imports, strava_connections};
use once_cell::sync::Lazy;
use prometheus::{register_int_gauge, Encoder, IntGauge, TextEncoder};
use sea_orm::{DbErr, EntityTrait, PaginatorTrait};
use std::path::Path;
use std::sync::Arc;

pub use bike_core::platform::api_metrics::metrics_middleware;
pub use bike_core::provider_metrics::{
    init_provider_metrics, record_provider_api_request, record_provider_rate_limit_pause,
    set_provider_rate_limit_bucket,
};

static STRAVA_CONNECTED_ATHLETES: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "bike_rust_strava_connected_athletes",
        "Current number of Strava athletes connected to Bike."
    )
    .expect("register Strava connected athletes gauge")
});

static UPLOADS_DISK_USAGE_BYTES: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "bike_rust_uploads_disk_usage_bytes",
        "Bytes stored under the activity upload directory."
    )
    .expect("register upload directory usage gauge")
});

static FAILED_ACTIVITY_IMPORTS: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "bike_rust_api_failed_activity_imports",
        "Stored activity imports currently failed and awaiting investigation or reprocessing."
    )
    .expect("register failed activity imports gauge")
});

/// Initialize all API metrics.  Must be called once at startup.
pub fn init_metrics() {
    bike_core::platform::api_metrics::init_api_metrics("bike_rust_api");
    init_provider_metrics();
    bike_core::strava_gateway_metrics::init();
    Lazy::force(&STRAVA_CONNECTED_ATHLETES);
    Lazy::force(&UPLOADS_DISK_USAGE_BYTES);
    Lazy::force(&FAILED_ACTIVITY_IMPORTS);
}

pub async fn metrics_route(State(state): State<Arc<AppStorage>>) -> Response {
    let status = match refresh_database_metrics(&state).await {
        Ok(()) => axum::http::StatusCode::OK,
        Err(error) => {
            tracing::warn!(error = ?error, "failed to refresh database-backed metrics");
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        }
    };
    refresh_upload_metrics(&state.uploads_dir).await;

    let encoder = TextEncoder::new();
    let mut metric_families = bike_core::platform::api_metrics::registry().gather();
    metric_families.extend(prometheus::gather());

    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .unwrap_or_default();
    let body = String::from_utf8(buffer).unwrap_or_default();

    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, encoder.format_type())
        .body(Body::from(body))
        .expect("failed to build metrics response")
}

async fn refresh_upload_metrics(root: &str) {
    let uploads_dir = root.to_owned();
    match tokio::task::spawn_blocking(move || directory_bytes(Path::new(&uploads_dir))).await {
        Ok(Ok(bytes)) => UPLOADS_DISK_USAGE_BYTES.set(bytes.min(i64::MAX as u64) as i64),
        Ok(Err(error)) => tracing::warn!(%error, "failed to measure activity upload directory"),
        Err(error) => tracing::warn!(%error, "activity upload directory scan failed"),
    }
}

fn directory_bytes(root: &Path) -> std::io::Result<u64> {
    let mut bytes = 0u64;
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let metadata = std::fs::symlink_metadata(entry.path())?;
            if metadata.is_dir() {
                directories.push(entry.path());
            } else if metadata.is_file() {
                bytes = bytes.saturating_add(metadata.len());
            }
        }
    }
    Ok(bytes)
}

async fn refresh_database_metrics(state: &AppStorage) -> Result<(), DbErr> {
    let connected_athletes = strava_connections::Entity::find()
        .count(&state.db)
        .await?
        .min(i64::MAX as u64) as i64;

    STRAVA_CONNECTED_ATHLETES.set(connected_athletes);
    FAILED_ACTIVITY_IMPORTS.set(
        activity_imports::Entity::failed_count(&state.db)
            .await?
            .min(i64::MAX as u64) as i64,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::directory_bytes;

    #[test]
    fn measures_nested_upload_files() {
        let root = std::env::temp_dir().join(format!("bike-upload-metrics-{}", std::process::id()));
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("nested/activity.fit"), b"12345").unwrap();
        assert_eq!(directory_bytes(&root).unwrap(), 5);
        std::fs::remove_dir_all(root).unwrap();
    }
}
