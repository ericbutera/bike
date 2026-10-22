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

pub const DETECTOR_VERSION: i32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub route_points: Vec<ActivityRoutePoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClimbOutput {
    pub detector_version: i32,
    pub recording_sha256: String,
    pub climbs: Vec<Climb>,
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
    Climb {
        climb_number: climb.climb_number,
        start_route_point_index: bounds.map(|(start, _)| start as i32),
        end_route_point_index: bounds.map(|(_, end)| end as i32),
        start_elapsed_seconds: climb.start_seconds,
        end_elapsed_seconds: climb.summit_seconds,
        duration_seconds: climb.duration_seconds,
        distance_meters: climb.distance_meters,
        elevation_gain_meters: climb.gain_meters,
        average_grade_percent: climb.average_grade_percent,
        route_points: bounds.map_or_else(Vec::new, |(start, end)| route[start..=end].to_vec()),
    }
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
