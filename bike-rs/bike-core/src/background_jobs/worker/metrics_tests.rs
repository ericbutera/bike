use super::*;
use axum::body::to_bytes;

#[tokio::test]
async fn unit_happy_completed_task_exports_counts_and_timings() {
    let metrics = WorkerMetrics::new("bike_unit");
    metrics.warmup_task_types(&["prepare_heatmap"]);
    metrics.record_invocation("prepare_heatmap");
    metrics.record_processing_lag("prepare_heatmap", 2.5);
    metrics.record_duration("prepare_heatmap", 0.25);
    metrics.record_completed("prepare_heatmap");
    let response = metrics.render_response();
    assert_eq!(response.status(), 200);
    assert!(response.headers()[CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/plain"));
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();

    for expected in [
        "bike_unit_task_invocations_total{type=\"prepare_heatmap\"} 1",
        "bike_unit_tasks_completed_total{type=\"prepare_heatmap\"} 1",
        "bike_unit_tasks_failed_total{type=\"prepare_heatmap\"} 0",
        "bike_unit_task_processing_lag_seconds_sum{type=\"prepare_heatmap\"} 2.5",
        "bike_unit_task_duration_seconds_sum{type=\"prepare_heatmap\"} 0.25",
    ] {
        assert!(text.contains(expected), "missing metric: {expected}");
    }
}
