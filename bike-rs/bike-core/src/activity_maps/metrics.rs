use once_cell::sync::Lazy;
use prometheus::{
    register_histogram_vec, register_int_counter, register_int_gauge, HistogramVec, IntCounter,
    IntGauge,
};

pub(super) struct Metrics {
    pub hits: IntCounter,
    pub misses: IntCounter,
    pub write_failures: IntCounter,
    pub pruned: IntCounter,
    pub bytes: IntGauge,
    pub images: IntGauge,
    pub duration: HistogramVec,
}

pub(super) static METRICS: Lazy<Metrics> = Lazy::new(|| Metrics {
    hits: register_int_counter!(
        "bike_maps_cache_hits_total",
        "Cached activity maps returned by Bike API."
    )
    .expect("register map cache hits"),
    misses: register_int_counter!(
        "bike_maps_cache_misses_total",
        "Activity map requests that missed the API cache."
    )
    .expect("register map cache misses"),
    write_failures: register_int_counter!(
        "bike_maps_cache_write_failures_total",
        "Generated maps that could not be cached."
    )
    .expect("register map cache write failures"),
    pruned: register_int_counter!(
        "bike_maps_cache_pruned_images_total",
        "Idle cached map files deleted."
    )
    .expect("register map cache pruned files"),
    bytes: register_int_gauge!(
        "bike_maps_cache_disk_usage_bytes",
        "Bytes in the API map cache directory."
    )
    .expect("register map cache bytes"),
    images: register_int_gauge!("bike_maps_cache_image_count", "Cached map PNG file count.")
        .expect("register map cache image count"),
    duration: register_histogram_vec!(
        "bike_maps_image_request_duration_seconds",
        "API map image request duration.",
        &["outcome"]
    )
    .expect("register map image duration"),
});
