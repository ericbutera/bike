//! Saved climb output shared by imports, reprocessing and the one-off backfill.
use crate::{
    activity_data::{deserialize_derived_activity_data, ActivityDerivedData, ActivityRoutePoint},
    bike_tasks::ActivityTask,
    entities::{activities, activity_climbs},
    object_store::{checksum, ObjectStore},
    services::reports,
    workflow_error::WorkflowError,
};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const DETECTOR_VERSION: i32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Climb {
    pub climb_number: i32,
    pub start_route_point_index: Option<i32>,
    pub end_route_point_index: Option<i32>,
    pub start_elapsed_seconds: i32,
    pub end_elapsed_seconds: i32,
    pub duration_seconds: i32,
    pub distance_meters: f64,
    pub elevation_gain_meters: f64,
    pub average_grade_percent: Option<f64>,
    #[serde(default)]
    pub elevation_loss_meters: Option<f64>,
    #[serde(default)]
    pub max_grade_percent: Option<f64>,
    #[serde(default)]
    pub category: Option<String>,
    pub route_points: Vec<ActivityRoutePoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ClimbOutput {
    pub detector_version: i32,
    pub recording_sha256: String,
    pub climbs: Vec<Climb>,
}

/// Return saved climb rows; recording and processing metadata do not gate reads.
pub async fn load(db: &DatabaseConnection, activity: &activities::Model) -> Result<Option<ClimbOutput>, WorkflowError> {
    let output = activity_climbs::output::Model::for_activity(db, activity.id).await?;
    let rows = activity_climbs::Model::for_activity(db, activity.id).await?;
    Ok(Some(ClimbOutput {
        detector_version: output.as_ref().map_or(0, |output| output.detector_version),
        recording_sha256: output.map_or_else(String::new, |output| output.recording_sha256),
        climbs: rows.into_iter().map(activity_climbs::Model::into_climb).collect(),
    }))
}

fn recording_checksum(recording: &ActivityDerivedData) -> Result<String, WorkflowError> {
    serde_json::to_vec(recording)
        .map(|bytes| checksum(&bytes))
        .map_err(|error| WorkflowError::internal(error.to_string()))
}

fn climb_from_detection(climb: reports::ClimbResponse, route: &[ActivityRoutePoint]) -> Climb {
    let start = route
        .iter()
        .position(|point| point.elapsed_seconds >= climb.start_seconds);
    let end = route
        .iter()
        .rposition(|point| point.elapsed_seconds <= climb.summit_seconds);
    let bounds = start.zip(end).filter(|(start, end)| start <= end);
    let route_points = bounds.map_or_else(Vec::new, |(start, end)| route[start..=end].to_vec());
    Climb {
        elevation_loss_meters: climb_elevation_loss(&route_points),
        max_grade_percent: climb_max_grade(&route_points),
        category: climb_category(climb.distance_meters, climb.average_grade_percent),
        climb_number: climb.climb_number,
        start_route_point_index: bounds.map(|(start, _)| start as i32),
        end_route_point_index: bounds.map(|(_, end)| end as i32),
        start_elapsed_seconds: climb.start_seconds,
        end_elapsed_seconds: climb.summit_seconds,
        duration_seconds: climb.duration_seconds,
        distance_meters: climb.distance_meters,
        elevation_gain_meters: climb.gain_meters,
        average_grade_percent: climb.average_grade_percent,
        route_points,
    }
}

fn climb_category(distance: f64, average_grade: Option<f64>) -> Option<String> {
    let grade = average_grade?;
    if distance < 250.0 || grade < 3.0 {
        return None;
    }
    let score = distance * grade;
    [
        (64_000.0, "HC"),
        (48_000.0, "1"),
        (32_000.0, "2"),
        (16_000.0, "3"),
        (8_000.0, "4"),
    ]
    .into_iter()
    .find(|(minimum, _)| score >= *minimum)
    .map(|(_, category)| category.into())
}

fn climb_elevation_loss(points: &[ActivityRoutePoint]) -> Option<f64> {
    (points.len() >= 2).then(|| {
        points
            .windows(2)
            .filter_map(|pair| {
                let previous = pair[0].elevation_meters?;
                let current = pair[1].elevation_meters?;
                (previous.is_finite() && current.is_finite()).then_some((previous - current).max(0.0))
            })
            .sum()
    })
}

fn distance_and_elevation(point: &ActivityRoutePoint) -> Option<(f64, f64)> {
    let distance = point.distance_meters?;
    let elevation = point.elevation_meters?;
    (distance.is_finite() && elevation.is_finite()).then_some((distance, elevation))
}

fn window_grade(points: &[ActivityRoutePoint], start_index: usize) -> Option<f64> {
    let (start_distance, start_elevation) = distance_and_elevation(&points[start_index])?;
    let (distance, elevation) = points[start_index + 1..]
        .iter()
        .filter_map(distance_and_elevation)
        .find(|(distance, _)| distance - start_distance >= 60.0)?;
    let grade = (elevation - start_elevation) / (distance - start_distance) * 100.0;
    grade.is_finite().then_some(grade)
}

fn interval_grade(pair: &[ActivityRoutePoint]) -> Option<f64> {
    let (start_distance, start_elevation) = distance_and_elevation(&pair[0])?;
    let (end_distance, end_elevation) = distance_and_elevation(&pair[1])?;
    let distance = end_distance - start_distance;
    if distance <= 0.0 || pair[1].elapsed_seconds <= pair[0].elapsed_seconds {
        return None;
    }
    Some((end_elevation - start_elevation) / distance * 100.0)
}

fn climb_max_grade(points: &[ActivityRoutePoint]) -> Option<f64> {
    if points.len() < 2 {
        return None;
    }
    let window_max = (0..points.len() - 1)
        .filter_map(|index| window_grade(points, index))
        .reduce(f64::max);
    let max = window_max.or_else(|| points.windows(2).filter_map(interval_grade).reduce(f64::max));
    Some(max.unwrap_or(0.0).max(0.0))
}

pub async fn rebuild(db: &DatabaseConnection, objects: &ObjectStore, task: ActivityTask) -> Result<(), WorkflowError> {
    let Some(activity) = activities::Model::find_owned(db, task.activity_id, task.user_id).await? else {
        return Ok(());
    };
    let recording = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    let recording_sha256 = recording_checksum(&recording)?;
    if activity_climbs::output::Model::for_activity(db, activity.id)
        .await?
        .is_some_and(|output| output.matches(&recording_sha256))
    {
        return Ok(());
    }
    let output = ClimbOutput {
        detector_version: DETECTOR_VERSION,
        recording_sha256,
        climbs: reports::activity_climbs(&activity)
            .into_iter()
            .map(|climb| climb_from_detection(climb, &recording.route_points))
            .collect(),
    };
    let reference = objects
        .transform(
            &format!("activity-imports/derived/{}/climbs/v{DETECTOR_VERSION}", activity.id),
            &output,
        )
        .await?;
    activity_climbs::Model::replace_output(db, &activity, &output, &reference).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object_store::ObjectRef;
    use crate::{object_store::ObjectOwner, test_support as fixtures};
    use ::object_store::{ObjectStore as _, ObjectStoreExt};
    use futures::TryStreamExt;

    fn fixture_activity() -> activities::Model {
        let parsed = crate::activity_parser::parse_activity_data(
            "unmarked_01.fit",
            "fit",
            fixtures::recordings::UNMARKED_01_FIT,
        )
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
            recording_sha256: recording_checksum(&deserialize_derived_activity_data(
                activity.derived_data_json.as_ref(),
            ))
            .unwrap(),
            object_ref: serde_json::json!({}),
            climb_count: count,
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

    async fn retained_output(backend: &::object_store::memory::InMemory) -> (ObjectRef, ClimbOutput) {
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
            .map(|climb| climb_from_detection(climb, &recording.route_points))
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
            object_ref: serde_json::to_value(&reference).unwrap(),
            climb_count: 0,
        };
        let db = fixtures::postgres_mock()
            .append_query_results([vec![activity]])
            .append_query_results([vec![saved]])
            .into_connection();
        rebuild(&db, &objects, fixtures::activity_task()).await.unwrap();
        assert_eq!(db.into_transaction_log().len(), 2);
        let stored: ClimbOutput = objects.read_json(&reference).await.unwrap();
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
}
