use once_cell::sync::Lazy;
use prometheus::{register_histogram_vec, register_int_counter_vec, HistogramVec, IntCounterVec};
use std::time::Duration;

static REQUESTS: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "bike_rust_strava_gateway_requests_total",
        "Requests made by the Rust Bike site to the Strava gateway.",
        &["operation", "status_code"]
    )
    .expect("register Rust Strava gateway request counter")
});

static DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "bike_rust_strava_gateway_request_duration_seconds",
        "Strava gateway request duration in seconds.",
        &["operation", "status_code"],
        vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]
    )
    .expect("register Rust Strava gateway request duration")
});

pub fn init() {
    Lazy::force(&REQUESTS);
    Lazy::force(&DURATION);
    for operation in [
        "begin_connect",
        "connection_status",
        "queue_sync",
        "disconnect",
    ] {
        REQUESTS.with_label_values(&[operation, "200"]);
        REQUESTS.with_label_values(&[operation, "500"]);
        DURATION.with_label_values(&[operation, "200"]);
        DURATION.with_label_values(&[operation, "500"]);
    }
}

pub fn record_request(operation: &str, succeeded: bool, duration: Duration) {
    let operation = match operation {
        "begin_connect" | "connection_status" | "queue_sync" | "disconnect" => operation,
        _ => "other",
    };
    let status_code = if succeeded { "200" } else { "500" };
    REQUESTS.with_label_values(&[operation, status_code]).inc();
    DURATION
        .with_label_values(&[operation, status_code])
        .observe(duration.as_secs_f64());
}
