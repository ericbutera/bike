// Shared auth + HTTP metrics live in glass::api_metrics.
// This module is a thin wrapper that initialises the shared registry with the
// app namespace supplied by the generated project.

pub use kaleido::glass::api_metrics::{metrics_middleware, metrics_route};

/// Initialize all API metrics.  Must be called once at startup.
pub fn init_metrics() {
    kaleido::glass::api_metrics::init_api_metrics("bike_api");
}
