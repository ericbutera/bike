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
