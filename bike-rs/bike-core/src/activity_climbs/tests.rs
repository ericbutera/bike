use super::*;
use crate::object_store::ObjectRef;
use crate::{object_store::ObjectOwner, test_support as fixtures};
use ::object_store::{ObjectStore as _, ObjectStoreExt};
use futures::TryStreamExt;

fn fixture_activity() -> activities::Model {
    let parsed =
        crate::activity_parser::parse_activity_data("unmarked_01.fit", "fit", fixtures::recordings::UNMARKED_01_FIT)
            .unwrap();
    activities::Model {
        derived_data_json: Some(crate::activity_data::serialize_derived_activity_data(
            &parsed.derived_data,
        )),
        ..fixtures::activity()
    }
}

fn saved_output(activity: &activities::Model, count: i32) -> activity_climbs::output::Model {
    activity_climbs::output::Model {
        activity_id: activity.id,
        detector_version: DETECTOR_VERSION,
        recording_sha256: recording_checksum(&deserialize_derived_activity_data(activity.derived_data_json.as_ref()))
            .unwrap(),
        climb_count: count,
        ..fixtures::activity_climb_output()
    }
}

fn saved_climb(activity: &activities::Model) -> activity_climbs::Model {
    activity_climbs::Model {
        activity_id: activity.id,
        climb_number: 3,
        start_route_point_index: Some(0),
        end_route_point_index: Some(2),
        start_elapsed_seconds: 0,
        end_elapsed_seconds: 12,
        duration_seconds: 12,
        distance_meters: 100.0,
        elevation_gain_meters: 2.0,
        average_grade_percent: Some(2.0),
        elevation_loss_meters: Some(17.0),
        max_grade_percent: Some(42.0),
        category: Some("HC".into()),
        route_points_json: crate::activity_data::serialize_route_point_series(
            &deserialize_derived_activity_data(activity.derived_data_json.as_ref())
                .route_points
                .into_iter()
                .take(3)
                .collect::<Vec<_>>(),
        ),
        ..fixtures::activity_climb()
    }
}

#[tokio::test]
async fn saved_read_returns_rows_without_a_recording_or_current_metadata() {
    let activity = fixtures::activity();
    let row = saved_climb(&fixture_activity());
    let output = activity_climbs::output::Model {
        detector_version: 0,
        recording_sha256: "older-recording".into(),
        ..saved_output(&activity, 99)
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![output]])
        .append_query_results([vec![row.clone()]])
        .into_connection();
    let output = load(&db, &activity).await.unwrap().unwrap();
    assert_eq!(output.climbs, vec![row.into_climb()]);
    assert_eq!(output.climbs[0].elevation_loss_meters, Some(17.0));
    assert_eq!(output.climbs[0].max_grade_percent, Some(42.0));
    assert_eq!(output.climbs[0].category.as_deref(), Some("HC"));
    assert_eq!(output.climbs[0].route_points.len(), 3);
    let sql = format!("{:?}", db.into_transaction_log());
    assert!(!sql.contains("derived_data_json"));
    assert!(!sql.contains("INSERT"));
}

#[tokio::test]
async fn saved_rows_remain_available_without_output_metadata() {
    let activity = fixtures::activity();
    let row = saved_climb(&activity);
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<activity_climbs::output::Model>::new()])
        .append_query_results([vec![row.clone()]])
        .into_connection();
    assert_eq!(
        load(&db, &activity).await.unwrap().unwrap().climbs,
        vec![row.into_climb()]
    );
}

#[tokio::test]
async fn missing_and_completed_empty_results_both_return_empty_climbs() {
    let activity = fixtures::activity();
    for metadata in [vec![], vec![saved_output(&activity, 0)]] {
        let db = fixtures::postgres_mock()
            .append_query_results([metadata])
            .append_query_results([Vec::<activity_climbs::Model>::new()])
            .into_connection();
        assert!(load(&db, &activity).await.unwrap().unwrap().climbs.is_empty());
    }
}

#[tokio::test]
async fn saved_read_propagates_database_errors() {
    let db = fixtures::postgres_mock()
        .append_query_errors([sea_orm::DbErr::Custom("unavailable".into())])
        .into_connection();
    assert!(load(&db, &fixtures::activity())
        .await
        .unwrap_err()
        .message
        .contains("Database request failed"));
}

#[test]
fn legacy_rows_retain_saved_metrics_without_inventing_new_values_or_geometry() {
    let row = activity_climbs::Model {
        elevation_loss_meters: None,
        max_grade_percent: None,
        category: None,
        route_points_json: Default::default(),
        ..saved_climb(&fixtures::activity())
    };
    let climb = row.into_climb();
    assert_eq!(climb.elevation_gain_meters, 2.0);
    assert_eq!(climb.elevation_loss_meters, None);
    assert_eq!(climb.max_grade_percent, None);
    assert_eq!(climb.category, None);
    assert!(climb.route_points.is_empty());
}

fn metric_points() -> Vec<ActivityRoutePoint> {
    let mut points = deserialize_derived_activity_data(fixture_activity().derived_data_json.as_ref()).route_points;
    points.truncate(3);
    for (index, point) in points.iter_mut().enumerate() {
        point.distance_meters = Some(index as f64 * 60.0);
        point.elapsed_seconds = index as i32 * 10;
        point.elevation_meters = Some([100.0, 106.0, 102.0][index]);
    }
    points
}

#[test]
fn processing_prepares_loss_and_window_grade_from_climb_geometry() {
    let points = metric_points();
    assert_eq!(climb_elevation_loss(&points), Some(4.0));
    assert_eq!(climb_max_grade(&points), Some(10.0));
    assert_eq!(climb_elevation_loss(&[]), None);
    assert_eq!(climb_max_grade(&points[..1]), None);
}

#[test]
fn processing_uses_adjacent_grade_for_short_geometry_and_ignores_missing_elevation() {
    let mut points = metric_points();
    for (index, point) in points.iter_mut().enumerate() {
        point.distance_meters = Some(index as f64 * 10.0);
    }
    assert_eq!(climb_max_grade(&points), Some(60.0));
    for point in &mut points {
        point.elevation_meters = None;
    }
    assert_eq!(climb_elevation_loss(&points), Some(0.0));
    assert_eq!(climb_max_grade(&points), Some(0.0));
}

#[test]
fn processing_preserves_category_thresholds() {
    for (distance, category) in [
        (1600.0, "4"),
        (3200.0, "3"),
        (6400.0, "2"),
        (9600.0, "1"),
        (12800.0, "HC"),
    ] {
        assert_eq!(climb_category(distance, Some(5.0)).as_deref(), Some(category));
    }
    assert_eq!(climb_category(100.0, Some(20.0)), None);
    assert_eq!(climb_category(5000.0, Some(2.0)), None);
    assert_eq!(climb_category(5000.0, None), None);
    assert_eq!(climb_category(250.0, Some(3.0)), None);
}

fn objects() -> ObjectStore {
    fixtures::object_storage(ObjectOwner::ActivityImports).0
}

async fn retained_output(backend: &::object_store::memory::InMemory) -> (ObjectRef, PreparedOutput) {
    let entries = backend.list(None).try_collect::<Vec<_>>().await.unwrap();
    assert_eq!(entries.len(), 1);
    let bytes = backend.get(&entries[0].location).await.unwrap().bytes().await.unwrap();
    let reference = ObjectRef {
        path: entries[0].location.to_string(),
        sha256: checksum(&bytes),
        bytes: bytes.len(),
    };
    (reference, serde_json::from_slice(&bytes).unwrap())
}

#[test]
fn existing_recording_produces_ordered_climbs_with_original_route_bounds() {
    let activity = fixture_activity();
    let recording = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    let climbs: Vec<_> = reports::activity_climbs(&activity)
        .into_iter()
        .map(|climb| climb_from_detection(&climb, &recording.route_points))
        .collect();
    assert!(!climbs.is_empty());
    for (index, climb) in climbs.iter().enumerate() {
        assert_eq!(climb.climb_number, index as i32 + 1);
        assert_eq!(
            climb.duration_seconds,
            climb.end_elapsed_seconds - climb.start_elapsed_seconds
        );
        let start = climb.start_route_point_index.unwrap() as usize;
        let end = climb.end_route_point_index.unwrap() as usize;
        assert_eq!(climb.route_points, recording.route_points[start..=end]);
    }
}

#[tokio::test]
async fn empty_result_is_saved_and_repeated_processing_skips_it() {
    let activity = fixtures::activity();
    let (objects, backend) = fixtures::object_storage(ObjectOwner::ActivityImports);
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity.clone()]])
        .append_query_results([Vec::<activity_climbs::output::Model>::new()])
        .append_query_results([vec![activity.clone()]])
        .append_exec_results([fixtures::written(), fixtures::written()])
        .into_connection();
    rebuild(&db, &objects, fixtures::activity_task()).await.unwrap();
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("activity_climb_outputs"));
    assert!(log.contains("COMMIT"));
    let (reference, output) = retained_output(backend.as_ref()).await;
    assert!(output.climbs.is_empty());
    let saved = activity_climbs::output::Model {
        activity_id: activity.id,
        detector_version: DETECTOR_VERSION,
        recording_sha256: output.recording_sha256,
        object_path: reference.path.clone(),
        object_sha256: reference.sha256.clone(),
        object_bytes: reference.bytes as i64,
        climb_count: 0,
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity]])
        .append_query_results([vec![saved]])
        .into_connection();
    rebuild(&db, &objects, fixtures::activity_task()).await.unwrap();
    assert_eq!(db.into_transaction_log().len(), 2);
    let stored: PreparedOutput = objects.read_json(&reference).await.unwrap();
    assert!(stored.climbs.is_empty());
}

#[tokio::test]
async fn nonempty_output_saves_artifact_and_climb_rows_together() {
    let activity = fixture_activity();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity.clone()]])
        .append_query_results([Vec::<activity_climbs::output::Model>::new()])
        .append_query_results([vec![activity]])
        .append_exec_results([fixtures::written(), fixtures::written(), fixtures::written()])
        .into_connection();
    let (objects, backend) = fixtures::object_storage(ObjectOwner::ActivityImports);
    rebuild(&db, &objects, fixtures::activity_task()).await.unwrap();
    let (_, output) = retained_output(backend.as_ref()).await;
    assert!(!output.climbs.is_empty());
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains(r#"INSERT INTO \"activity_climbs\""#));
    assert!(log.contains("ON CONFLICT"));
    assert!(log.contains("COMMIT"));
}

#[tokio::test]
async fn source_change_prevents_stale_output_publication() {
    let activity = fixtures::activity();
    let changed = fixture_activity();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity]])
        .append_query_results([Vec::<activity_climbs::output::Model>::new()])
        .append_query_results([vec![changed]])
        .into_connection();
    let error = rebuild(&db, &objects(), fixtures::activity_task()).await.unwrap_err();
    assert!(error.message.contains("changed"));
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("ROLLBACK"));
    assert!(!log.contains("INSERT"));
}

fn report_activity() -> activities::Model {
    let mut recording = deserialize_derived_activity_data(None);
    recording.route_points = [
        (0, 0.0, 100.0, 120),
        (30, 125.0, 106.0, 130),
        (60, 250.0, 114.0, 142),
        (90, 390.0, 122.0, 152),
        (120, 520.0, 126.0, 160),
        (150, 560.0, 123.0, 154),
        (160, 590.0, 120.0, 150),
        (180, 640.0, 118.0, 142),
    ]
    .into_iter()
    .map(|(elapsed_seconds, distance, elevation, hr)| ActivityRoutePoint {
        elapsed_seconds,
        latitude: 42.0,
        longitude: -83.0,
        distance_meters: Some(distance),
        elevation_meters: Some(elevation),
        speed_mps: None,
        heart_rate_bpm: Some(hr),
        cadence_rpm: Some(80),
        power_watts: Some(220),
    })
    .collect();
    activities::Model {
        derived_data_json: Some(crate::activity_data::serialize_derived_activity_data(&recording)),
        ..fixtures::activity()
    }
}

#[tokio::test]
async fn climb_stage_retains_existing_report_metrics_and_recovery_after_the_summit() {
    let activity = report_activity();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity.clone()]])
        .append_query_results([Vec::<activity_climbs::output::Model>::new()])
        .append_query_results([vec![activity]])
        .append_exec_results([fixtures::written(), fixtures::written(), fixtures::written()])
        .into_connection();
    let (objects, backend) = fixtures::object_storage(ObjectOwner::ActivityImports);
    rebuild(&db, &objects, fixtures::activity_task()).await.unwrap();
    let (_, output) = retained_output(backend.as_ref()).await;
    assert_eq!(output.climbs.len(), 1);
    let saved = &output.climbs[0];
    assert_eq!(saved.climb.distance_meters, 520.0);
    assert_eq!(saved.climb.elevation_gain_meters, 26.0);
    assert_eq!(saved.climb.route_points.last().unwrap().elapsed_seconds, 120);
    assert_eq!(saved.facts.vertical_rate_meters_per_hour, 780.0);
    assert_eq!(saved.facts.average_speed_mps, Some(4.33));
    assert_eq!(saved.facts.average_heart_rate_bpm, Some(140.8));
    assert_eq!(saved.facts.peak_heart_rate_bpm, Some(160));
    assert_eq!(saved.facts.average_cadence_rpm, Some(80.0));
    assert_eq!(saved.facts.average_power_watts, Some(220.0));
    assert_eq!(saved.facts.heart_rate_recovery_30_seconds_bpm, Some(6));
    assert_eq!(saved.facts.heart_rate_recovery_60_seconds_bpm, Some(18));
    assert_eq!(saved.facts.seconds_to_drop_10_bpm, Some(40));
    assert_eq!(saved.facts.seconds_to_drop_15_bpm, Some(60));
    assert!(saved.facts.summit_immediately_enters_descent);
    assert!(!saved.facts.is_first_half);
}

#[test]
fn real_recording_preserves_every_existing_report_fact_in_recording_order() {
    let activity = fixture_activity();
    let route = deserialize_derived_activity_data(activity.derived_data_json.as_ref()).route_points;
    for report in reports::activity_climbs(&activity) {
        let expected = serde_json::to_value(&report).unwrap();
        let prepared = PreparedClimb::from_detection(report, &route);
        let mut facts = serde_json::to_value(&prepared.facts).unwrap();
        assert_eq!(
            facts.as_object_mut().unwrap().remove("is_first_half").unwrap(),
            expected["first_or_second_half"] == "first"
        );
        for (name, value) in facts.as_object().unwrap() {
            assert_eq!(value, &expected[name], "report fact {name}");
        }
    }
}

#[test]
fn absent_sensors_remain_missing_instead_of_becoming_zero() {
    let mut activity = report_activity();
    let mut recording = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    for point in &mut recording.route_points {
        point.heart_rate_bpm = None;
        point.cadence_rpm = None;
        point.power_watts = None;
    }
    activity.derived_data_json = Some(crate::activity_data::serialize_derived_activity_data(&recording));
    let prepared =
        PreparedClimb::from_detection(reports::activity_climbs(&activity).remove(0), &recording.route_points);
    assert_eq!(prepared.facts.average_heart_rate_bpm, None);
    assert_eq!(prepared.facts.peak_heart_rate_bpm, None);
    assert_eq!(prepared.facts.average_cadence_rpm, None);
    assert_eq!(prepared.facts.average_power_watts, None);
    assert_eq!(prepared.facts.heart_rate_recovery_30_seconds_bpm, None);
    assert_eq!(prepared.facts.heart_rate_recovery_60_seconds_bpm, None);
    assert_eq!(prepared.facts.seconds_to_drop_10_bpm, None);
    assert_eq!(prepared.facts.seconds_to_drop_15_bpm, None);
}

#[tokio::test]
async fn older_output_is_rebuilt_even_when_the_recording_is_unchanged() {
    let activity = report_activity();
    let old = activity_climbs::output::Model {
        detector_version: 2,
        ..saved_output(&activity, 1)
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity.clone()]])
        .append_query_results([vec![old]])
        .append_query_results([vec![activity]])
        .append_exec_results([fixtures::written(), fixtures::written(), fixtures::written()])
        .into_connection();
    let (objects, backend) = fixtures::object_storage(ObjectOwner::ActivityImports);
    rebuild(&db, &objects, fixtures::activity_task()).await.unwrap();
    let (_, output) = retained_output(backend.as_ref()).await;
    assert_eq!(output.detector_version, 3);
    assert_eq!(output.climbs[0].facts.heart_rate_recovery_60_seconds_bpm, Some(18));
    let sql = format!("{:?}", db.into_transaction_log());
    assert!(sql.contains("object_path"));
    assert!(!sql.contains("object_ref"));
    assert!(!sql.contains("segment_efforts"));
    assert!(!sql.contains("activity_earned_awards"));
}

#[tokio::test]
async fn a_climb_task_for_another_owner_does_not_read_or_publish_output() {
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<activities::Model>::new()])
        .into_connection();
    let (objects, backend) = fixtures::object_storage(ObjectOwner::ActivityImports);
    rebuild(
        &db,
        &objects,
        ActivityTask {
            user_id: 99,
            ..fixtures::activity_task()
        },
    )
    .await
    .unwrap();
    assert!(backend.list(None).try_collect::<Vec<_>>().await.unwrap().is_empty());
    let log = db.into_transaction_log();
    assert_eq!(log.len(), 1);
    assert!(format!("{log:?}").contains("99"));
}

#[tokio::test]
async fn unchanged_nonempty_retry_uses_the_typed_reference_without_writing_again() {
    let activity = report_activity();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity.clone()]])
        .append_query_results([Vec::<activity_climbs::output::Model>::new()])
        .append_query_results([vec![activity.clone()]])
        .append_exec_results([fixtures::written(), fixtures::written(), fixtures::written()])
        .into_connection();
    let (objects, backend) = fixtures::object_storage(ObjectOwner::ActivityImports);
    rebuild(&db, &objects, fixtures::activity_task()).await.unwrap();
    let (reference, output) = retained_output(backend.as_ref()).await;
    let saved = activity_climbs::output::Model {
        recording_sha256: output.recording_sha256.clone(),
        object_path: reference.path,
        object_sha256: reference.sha256,
        object_bytes: reference.bytes as i64,
        climb_count: 1,
        ..fixtures::activity_climb_output()
    };
    let reference = saved.object_reference().unwrap();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity]])
        .append_query_results([vec![saved]])
        .into_connection();
    rebuild(&db, &objects, fixtures::activity_task()).await.unwrap();
    assert_eq!(db.into_transaction_log().len(), 2);
    let stored: PreparedOutput = objects.read_json(&reference).await.unwrap();
    assert_eq!(stored, output);
    assert_eq!(retained_output(backend.as_ref()).await.0, reference);
}
