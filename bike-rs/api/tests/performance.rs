//! PERF08: small fixture and real HTTP measurements. No application optimization.
#[path = "performance/checks.rs"]
mod checks;
#[path = "performance/dataset.rs"]
mod dataset;
#[path = "performance/measure.rs"]
mod measure;
#[path = "performance/recording.rs"]
mod recording;
#[path = "performance/seed.rs"]
mod seed;

#[tokio::test]
#[ignore = "requires the isolated stack from mise run perf:small"]
async fn seed_small() {
    let manifest = dataset::manifest();
    let db = seed::connect().await;
    seed::seed(&db).await;
    let directory = std::env::var("BIKE_PERF_OUTPUT").unwrap();
    std::fs::write(
        format!("{directory}/manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
#[ignore = "requires the isolated stack from mise run perf:small"]
async fn baseline() {
    let db = seed::connect().await;
    let base_url = std::env::var("BIKE_PERF_API_URL").unwrap();
    let report = measure::measure(&db, &base_url).await;
    let directory = std::env::var("BIKE_PERF_OUTPUT").unwrap();
    std::fs::write(
        format!("{directory}/measurements.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    for stage in [&report, &report["grown_history"]] {
        if stage.is_null() {
            continue;
        }
        assert!(stage["runs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|run| run["errors"] == 0 && run["requests"] == 200));
    }
}

#[test]
fn small_fixture_has_repeated_valid_geometry_and_ties() {
    let activity = dataset::activity(1, 100);
    let data = bike_core::activity_details::deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    assert_eq!(data.route_points.len(), 201);
    for occurrence in 1..=2 {
        let effort = dataset::effort(1, 100, occurrence);
        let start = &data.route_points[effort.start_route_point_index as usize];
        let end = &data.route_points[effort.end_route_point_index as usize];
        assert_eq!(end.elapsed_seconds - start.elapsed_seconds, effort.duration_seconds);
        assert_eq!(start.latitude, data.route_points[0].latitude);
        assert_eq!(end.latitude, data.route_points[50].latitude);
    }
    assert_eq!(
        dataset::effort(1, 95, 1).duration_seconds,
        dataset::effort(1, 100, 2).duration_seconds
    );
    assert_eq!(activity, dataset::activity(1, 100));
    assert_ne!(dataset::owner(1).pid, dataset::owner(2).pid);
}

#[test]
fn report_percentiles_use_nearest_rank() {
    let values: Vec<_> = (1..=200).map(f64::from).collect();
    assert_eq!(measure::percentile(&values, 50), 100.0);
    assert_eq!(measure::percentile(&values, 95), 190.0);
    assert_eq!(measure::percentile(&[7.0], 95), 7.0);
}

#[test]
fn correctness_rejects_truncated_or_cross_owner_responses() {
    let items: Vec<_> = (1081..=1100).map(|id| serde_json::json!({"id": id})).collect();
    let mut body = serde_json::json!({"metadata": {"total": 100}, "data": items});
    assert!(checks::valid("activity_list", &body));
    body["data"][0]["id"] = 2100.into();
    assert!(!checks::valid("activity_list", &body));
    assert!(!checks::valid(
        "segment_comparison",
        &serde_json::json!({"segment_id": 1})
    ));
}
