use crate::storage::AppStorage;
use axum::body::{to_bytes, Body};
use axum::http::Request;
use axum::response::Response;
use axum::Router;
use bike_core::entities::{activities, activity_climbs, activity_training_analyses, segment_efforts};
use bike_core::test_support;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

struct Fixture {
    app: Router,
    _uploads: TempDir,
    requests: Arc<std::sync::Mutex<test_support::temporal_client::Requests>>,
}

impl Fixture {
    async fn with_database(db: DatabaseConnection) -> Self {
        let uploads = tempfile::tempdir().unwrap();
        std::fs::create_dir(uploads.path().join("activity-imports")).unwrap();
        std::fs::write(uploads.path().join("activity-imports/fixture.gpx"), "<gpx/>").unwrap();
        let mut state = AppStorage::for_test(db.clone());
        state.local_admin_user_pid = Some(test_support::customer().pid);
        state.uploads_dir = uploads.path().to_str().unwrap().into();
        let requests = Default::default();
        state.tasks = bike_core::jobs::JobQueue::with_client(
            db,
            test_support::temporal_client::client(Arc::clone(&requests)).await,
        );
        Self {
            app: super::routes().with_state(Arc::new(state)),
            _uploads: uploads,
            requests,
        }
    }

    async fn request(&self, method: &str, path: &str, body: Value) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn json(&self, method: &str, path: &str, body: Value, status: u16) -> Value {
        let response = self.request(method, path, body).await;
        let actual_status = response.status();
        let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
        assert_eq!(
            actual_status,
            status,
            "{method} {path}: {}",
            String::from_utf8_lossy(&bytes)
        );
        serde_json::from_slice(&bytes).unwrap()
    }
}

#[tokio::test]
async fn owned_activity_update_preserves_fields_and_commits_its_audit() {
    use sea_orm::{DbBackend, MockDatabase, MockExecResult};
    for fail_start in [false, true] {
        let before = test_support::activity();
        let after = activities::Model {
            title: "Lunch Loop".into(),
            activity_type: "race".into(),
            ..before.clone()
        };
        let recording = bike_core::activity_data::deserialize_derived_activity_data(after.derived_data_json.as_ref());
        let output = activity_climbs::output::Model {
            activity_id: after.id,
            detector_version: bike_core::activity_climbs::DETECTOR_VERSION,
            recording_sha256: bike_core::object_store::checksum(&serde_json::to_vec(&recording).unwrap()),
            object_ref: json!({}),
            climb_count: 0,
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![test_support::customer()]])
            .append_query_results([vec![before.clone()], vec![before]])
            .append_query_results([vec![after.clone()]])
            .append_query_results([Vec::<segment_efforts::Model>::new()])
            .append_query_results([Vec::<activity_training_analyses::Model>::new()])
            .append_query_results([vec![output]])
            .append_query_results([Vec::<activity_climbs::Model>::new()])
            .append_query_results([vec![test_support::activity_earned_awards()]])
            .append_exec_results([MockExecResult {
                rows_affected: 1,
                ..Default::default()
            }])
            .into_connection();
        let fixture = Fixture::with_database(db.clone()).await;
        fixture.requests.lock().unwrap().fail_start = fail_start;
        let updated = fixture
            .json(
                "PATCH",
                "/api/activities/42",
                json!({"title": "  Lunch Loop  ", "activity_type": "race"}),
                if fail_start { 500 } else { 200 },
            )
            .await;
        if !fail_start {
            assert_eq!(updated["climb_output"]["climbs"], json!([]));
            assert_eq!(
                updated["earned_awards"],
                json!([test_support::activity_earned_awards()])
            );
            assert_eq!(updated["title"], after.title);
            assert_eq!(updated["activity_type"], after.activity_type);
            assert_eq!(updated["source"], after.source);
        }
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("FOR UPDATE"));
        assert!(log.contains("activity_event_history"));
        assert!(log.contains("COMMIT"));
        let requests = fixture.requests.lock().unwrap();
        assert_eq!(requests.starts.len(), 1);
        assert_eq!(
            requests.starts[0].workflow_type.as_ref().unwrap().name,
            "prepare_heatmap"
        );
        let input: bike_core::bike_tasks::ActivityTask =
            serde_json::from_slice(&requests.starts[0].input.as_ref().unwrap().payloads[0].data).unwrap();
        assert_eq!((input.user_id, input.activity_id), (after.user_id, after.id));
    }
}

#[tokio::test]
async fn creator_rename_preserves_geometry_and_saved_results_without_background_work() {
    use bike_core::entities::segments;
    let before = segments::Model {
        source_activity_id: Some(42),
        source_start_route_point_index: Some(1),
        source_end_route_point_index: Some(2),
        route_data_json: Some(bike_core::segment_support::serialize_segment_route_points(
            &test_support::parsed_reprocess_activity().derived_data.route_points,
        )),
        ..test_support::segment()
    };
    let after = segments::Model {
        title: "New name".into(),
        ..before.clone()
    };
    let db = test_support::postgres_mock()
        .append_query_results([vec![test_support::customer()]])
        .append_query_results([vec![before.clone()], vec![after.clone()]])
        .append_query_results([vec![test_support::segment_summary()]])
        .append_query_results([vec![test_support::segment_user_summary()]])
        .into_connection();
    let fixture = Fixture::with_database(db.clone()).await;
    let response = fixture
        .json("PUT", "/api/segments/1", json!({"title": "  New name  "}), 200)
        .await;
    assert_eq!(response["title"], after.title);
    assert_eq!(response["distance_meters"], before.distance_meters.unwrap());
    assert_eq!(response["builder_source"]["start_route_point_index"], 1);
    assert_eq!(response["builder_source"]["end_route_point_index"], 2);
    assert_eq!(response["effort_count"], test_support::segment_summary().effort_count);
    assert!(fixture.requests.lock().unwrap().starts.is_empty());
    let log = db.into_transaction_log();
    let writes: Vec<_> = log
        .iter()
        .flat_map(|transaction| transaction.statements())
        .filter(|statement| statement.sql.starts_with("UPDATE"))
        .collect();
    assert_eq!(writes.len(), 1);
    let assignment = writes[0].sql.split(" WHERE ").next().unwrap();
    assert!(assignment.contains("\"title\""));
    for field in [
        "route_data_json",
        "distance_meters",
        "source_activity_id",
        "source_start_route_point_index",
        "source_end_route_point_index",
        "last_activity_change_at",
        "mode",
        "starred",
    ] {
        assert!(
            !assignment.contains(&format!("\"{field}\" =")),
            "rename changed {field}"
        );
    }
    for table in [
        "segment_summaries",
        "segment_user_summaries",
        "segment_efforts",
        "activity_analytics",
        "activity_earned_awards",
    ] {
        assert!(!log
            .iter()
            .flat_map(|transaction| transaction.statements())
            .any(|statement| statement.sql.starts_with("DELETE")
                || statement.sql.starts_with(&format!("UPDATE \"{table}\""))));
    }
}

#[tokio::test]
async fn segment_metadata_rejects_geometry_fields() {
    for field in [
        "route_data_json",
        "route_points",
        "distance_meters",
        "source_activity_id",
        "start_route_point_index",
        "end_route_point_index",
        "source_start_route_point_index",
        "source_end_route_point_index",
    ] {
        let db = test_support::postgres_mock()
            .append_query_results([vec![test_support::customer()]])
            .into_connection();
        let fixture = Fixture::with_database(db).await;
        let response = fixture
            .request("PUT", "/api/segments/1", json!({"title": "New name", field: 2}))
            .await;
        assert_eq!(response.status(), 422, "accepted geometry field {field}");
        assert!(fixture.requests.lock().unwrap().starts.is_empty());
    }
}

#[tokio::test]
async fn saved_segment_route_cannot_be_replaced_through_the_old_builder_endpoint() {
    let fixture = Fixture::with_database(test_support::postgres_mock().into_connection()).await;
    let response = fixture
        .request(
            "PUT",
            "/api/segments/1/from-activity",
            json!({
                "activity_id": 42, "title": "Replacement", "start_route_point_index": 0, "end_route_point_index": 2
            }),
        )
        .await;
    assert_eq!(response.status(), 404);
    assert!(fixture.requests.lock().unwrap().starts.is_empty());
}

#[tokio::test]
async fn duplicate_builder_creation_keeps_the_existing_name_and_source_without_background_work() {
    use bike_core::entities::segments;
    let recording = test_support::parsed_reprocess_activity().derived_data;
    let selected = segments::slice_builder_route_points(&recording.route_points, 0, 2).unwrap();
    let existing = segments::Model {
        title: "Saved creator name".into(),
        source_activity_id: Some(99),
        source_start_route_point_index: Some(5),
        source_end_route_point_index: Some(7),
        distance_meters: selected.last().and_then(|point| point.distance_meters),
        route_data_json: Some(bike_core::segment_support::serialize_segment_route_points(&selected)),
        ..test_support::segment()
    };
    let activity = activities::Model {
        derived_data_json: Some(bike_core::activity_data::serialize_derived_activity_data(&recording)),
        ..test_support::activity()
    };
    let db = test_support::postgres_mock()
        .append_query_results([vec![test_support::customer()]])
        .append_query_results([vec![activity]])
        .append_query_results([vec![existing.clone()], vec![existing.clone()]])
        .append_query_results([vec![test_support::segment_summary()]])
        .append_query_results([vec![test_support::segment_user_summary()]])
        .into_connection();
    let fixture = Fixture::with_database(db.clone()).await;
    let response = fixture
        .json(
            "POST",
            "/api/segments/from-activity",
            json!({
                "activity_id": 42, "title": "Duplicate crop name", "start_route_point_index": 0,
                "end_route_point_index": 2
            }),
            200,
        )
        .await;
    assert_eq!(response["id"], existing.id);
    assert_eq!(response["title"], existing.title);
    assert_eq!(response["builder_source"]["activity_id"], 99);
    assert_eq!(response["builder_source"]["start_route_point_index"], 5);
    assert_eq!(response["builder_source"]["end_route_point_index"], 7);
    assert!(fixture.requests.lock().unwrap().starts.is_empty());
    assert!(db
        .into_transaction_log()
        .iter()
        .flat_map(|transaction| transaction.statements())
        .all(|statement| statement.sql.starts_with("SELECT")));
}
