use crate::activity_import_history::tests::{database, imported};
use crate::storage::AppStorage;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use bike_core::auth::entities::users;
use bike_core::entities::{activity_import_attempts, activity_imports};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Schema, Set};
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

struct Fixture {
    state: AppStorage,
    _uploads: TempDir,
}

impl Fixture {
    async fn new(admin: Option<bool>) -> Self {
        let db = database().await;
        let schema = Schema::new(db.get_database_backend());
        db.execute(&schema.create_table_from_entity(users::Entity))
            .await
            .unwrap();
        let pid = Uuid::new_v4();
        insert_user(&db, pid, admin.unwrap_or(false)).await;
        let uploads = tempfile::tempdir().unwrap();
        std::fs::write(uploads.path().join("ride.gpx"), "<gpx/>").unwrap();
        for mut import in [imported(1, 7), imported(2, 8), imported(3, 1)] {
            import.storage_path = "ride.gpx".into();
            let active: activity_imports::ActiveModel = import.into();
            active.insert(&db).await.unwrap();
        }
        let mut state = AppStorage::for_test(db);
        state.uploads_dir = uploads.path().to_str().unwrap().into();
        state.local_admin_user_pid = admin.map(|_| pid);
        Self {
            state,
            _uploads: uploads,
        }
    }

    fn router(&self) -> Router {
        crate::controllers::routes().with_state(Arc::new(self.state.clone()))
    }
}

async fn insert_user(db: &sea_orm::DatabaseConnection, pid: Uuid, admin: bool) {
    users::ActiveModel {
        id: Set(1),
        pid: Set(pid),
        email: Set("admin-imports@example.test".into()),
        api_key: Set("fixture-api-key".into()),
        name: Set("Fixture user".into()),
        is_admin: Set(Some(admin)),
        disabled: Set(false),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
}

fn request(method: &str, path: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(json!({"stage":"activity_saved"}).to_string()))
        .unwrap()
}

const ADMIN_REQUESTS: [(&str, &str); 4] = [
    ("GET", "/api/admin/activity-imports/history"),
    ("GET", "/api/admin/activity-imports/1/trace"),
    (
        "GET",
        "/api/admin/activity-imports/1/replay?stage=activity_saved",
    ),
    ("POST", "/api/admin/activity-imports/1/replay"),
];

#[tokio::test]
async fn history_and_replay_require_an_authenticated_administrator() {
    for (role, expected) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some(false), StatusCode::FORBIDDEN),
    ] {
        let fixture = Fixture::new(role).await;
        for (method, path) in ADMIN_REQUESTS {
            let response = fixture
                .router()
                .oneshot(request(method, path))
                .await
                .unwrap();
            assert_eq!(response.status(), expected, "{method} {path}");
        }
    }
}

#[tokio::test]
async fn rider_history_trace_and_replay_routes_are_removed() {
    let fixture = Fixture::new(Some(true)).await;
    for (method, path) in [
        ("GET", "/api/activity-imports/history"),
        ("GET", "/api/activity-imports/1/trace"),
        ("GET", "/api/activity-imports/1/replay?stage=activity_saved"),
        ("POST", "/api/activity-imports/1/replay"),
    ] {
        let response = fixture
            .router()
            .oneshot(request(method, path))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
    }
}

async fn get_json(router: Router, path: &str) -> Value {
    let response = router.oneshot(request("GET", path)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap()
}

#[tokio::test]
async fn administrator_history_defaults_to_self_and_can_select_another_owner() {
    let fixture = Fixture::new(Some(true)).await;
    let own = get_json(fixture.router(), "/api/admin/activity-imports/history").await;
    assert_eq!(own["total"], 1);
    assert_eq!(own["items"][0]["id"], 3);
    let other = get_json(
        fixture.router(),
        "/api/admin/activity-imports/history?user_id=7&source=manual_upload&status=failed",
    )
    .await;
    assert_eq!(other["total"], 1);
    assert_eq!(other["items"][0]["id"], 1);
    let response = fixture
        .router()
        .oneshot(request(
            "GET",
            "/api/admin/activity-imports/history?user_id=0",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn administrator_replay_queues_another_owners_import_without_changing_ownership() {
    let fixture = Fixture::new(Some(true)).await;
    let plan = get_json(
        fixture.router(),
        "/api/admin/activity-imports/1/replay?stage=activity_saved",
    )
    .await;
    assert_eq!(plan["start_stage"], "raw_stored");
    let response = fixture
        .router()
        .oneshot(request("POST", "/api/admin/activity-imports/1/replay"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let queued: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
    let attempt =
        activity_import_attempts::Entity::find_by_id(queued["attempt_id"].as_i64().unwrap() as i32)
            .one(&fixture.state.db)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(attempt.user_id, 7);
    let import = activity_imports::Entity::find_by_id(1)
        .one(&fixture.state.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(import.user_id, 7);
    let inspected =
        crate::activity_import_history::admin_import_for_inspection(&fixture.state.db, 1)
            .await
            .unwrap();
    assert_eq!(inspected.user_id, 7);
    let missing = fixture
        .router()
        .oneshot(request(
            "GET",
            "/api/admin/activity-imports/999/replay?stage=activity_saved",
        ))
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn synthetic_credentials_do_not_authorize_administrative_history() {
    let mut fixture = Fixture::new(Some(true)).await;
    let key = "fixture-synthetic-key-32-characters";
    fixture.state.synthetic_auth = Some(bike_core::synthetics::SyntheticAuth {
        key: key.into(),
        user_pid: fixture.state.local_admin_user_pid.unwrap(),
    });
    let request = Request::builder()
        .uri("/api/admin/activity-imports/history")
        .header(bike_core::synthetics::AUTH_HEADER, key)
        .body(Body::empty())
        .unwrap();
    let response = fixture.router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
