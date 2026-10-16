use crate::storage::AppStorage;
use axum::body::{to_bytes, Body};
use axum::http::Request;
use axum::response::Response;
use axum::Router;
use bike_core::entities::{activities, activity_training_analyses, segment_efforts};
use bike_core::test_support;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

struct Fixture {
    app: Router,
    _uploads: TempDir,
}

impl Fixture {
    fn with_database(db: DatabaseConnection) -> Self {
        let uploads = tempfile::tempdir().unwrap();
        std::fs::create_dir(uploads.path().join("activity-imports")).unwrap();
        std::fs::write(
            uploads.path().join("activity-imports/fixture.gpx"),
            "<gpx/>",
        )
        .unwrap();
        let mut state = AppStorage::for_test(db.clone());
        state.local_admin_user_pid = Some(test_support::customer().pid);
        state.uploads_dir = uploads.path().to_str().unwrap().into();
        Self {
            app: super::routes().with_state(Arc::new(state)),
            _uploads: uploads,
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
    let before = test_support::activity();
    let after = activities::Model {
        title: "Lunch Loop".into(),
        activity_type: "race".into(),
        ..before.clone()
    };
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([vec![test_support::customer()]])
        .append_query_results([vec![before.clone()], vec![before]])
        .append_query_results([vec![after.clone()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_query_results([Vec::<activity_training_analyses::Model>::new()])
        .append_exec_results([MockExecResult {
            rows_affected: 1,
            ..Default::default()
        }])
        .into_connection();
    let fixture = Fixture::with_database(db.clone());
    let updated = fixture
        .json(
            "PATCH",
            "/api/activities/42",
            json!({"title": "  Lunch Loop  ", "activity_type": "race"}),
            200,
        )
        .await;
    assert_eq!(updated["title"], after.title);
    assert_eq!(updated["activity_type"], after.activity_type);
    assert_eq!(updated["source"], after.source);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("FOR UPDATE"));
    assert!(log.contains("activity_event_history"));
    assert!(log.contains("COMMIT"));
}
