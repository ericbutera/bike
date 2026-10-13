use super::{files::StoredFile, protection::Protection};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use prometheus::{
    register_int_counter, register_int_gauge, register_int_gauge_vec, IntCounter, IntGauge,
    IntGaugeVec,
};
use std::collections::BTreeSet;

static FILES: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "bike_rust_strava_storage_files",
        "Current Strava files from a successful filesystem scan.",
        &["kind"]
    )
    .expect("register Strava storage files")
});
static BYTES: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "bike_rust_strava_storage_bytes",
        "Current logical Strava file bytes from a successful filesystem scan.",
        &["kind"]
    )
    .expect("register Strava storage bytes")
});
static SOURCES: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "bike_rust_strava_storage_protected_source_files",
        "Referenced JSON and original recording files retained without TTL."
    )
    .expect("register Strava protected sources")
});
static LAST_SUCCESS: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "bike_rust_strava_storage_last_success_timestamp_seconds",
        "Unix timestamp of the last completed Strava storage retention scan."
    )
    .expect("register Strava storage scan time")
});
static PRUNED_FILES: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "bike_rust_strava_storage_pruned_files_total",
        "Expired Strava files deleted by this worker."
    )
    .expect("register Strava pruned files")
});
static PRUNED_BYTES: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "bike_rust_strava_storage_pruned_bytes_total",
        "Logical bytes of expired Strava files deleted by this worker."
    )
    .expect("register Strava pruned bytes")
});

pub(super) fn pruned(bytes: u64) {
    PRUNED_FILES.inc();
    PRUNED_BYTES.inc_by(bytes);
}

pub(super) fn init() {
    Lazy::force(&PRUNED_FILES);
    Lazy::force(&PRUNED_BYTES);
}

pub(super) fn publish(
    inventory: &[StoredFile],
    sources: &BTreeSet<String>,
    protected: &Protection,
    cutoff: DateTime<Utc>,
) {
    for kind in ["source", "raw", "transform", "metadata"] {
        let files: Vec<_> = inventory
            .iter()
            .filter(|file| !file.expires(protected, cutoff) && file.kind(sources) == kind)
            .collect();
        FILES
            .with_label_values(&[kind])
            .set(files.len().min(i64::MAX as usize) as i64);
        BYTES.with_label_values(&[kind]).set(
            files
                .iter()
                .map(|file| file.bytes)
                .sum::<u64>()
                .min(i64::MAX as u64) as i64,
        );
    }
    SOURCES.set(sources.len().min(i64::MAX as usize) as i64);
    LAST_SUCCESS.set(Utc::now().timestamp());
    Lazy::force(&PRUNED_FILES);
    Lazy::force(&PRUNED_BYTES);
}
