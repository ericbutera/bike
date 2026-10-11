//! Native webhook HTTP intake, queued worker processor, PostgreSQL and fake Strava.
use axum::{
    extract::{Path, Query, State},
    http::{Request, StatusCode},
    routing::{get, post},
    Json, Router,
};
use bike_core::{
    auth::entities::users,
    background_jobs::{background_tasks, worker::TaskProcessor},
    config::Config,
    entities::{activities, integration_events, strava_connections, strava_workflows},
    strava::StravaWebhookEvent,
    strava_workflow,
};
use chrono::{Duration, Utc};
use migration::{Migrator, MigratorTrait};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, Set,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicU16, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tower::ServiceExt;

#[derive(Default)]
struct Vendor {
    activity: Mutex<Value>,
    streams: Mutex<Value>,
    detail_calls: AtomicUsize,
    stream_calls: AtomicUsize,
    list_calls: AtomicUsize,
    detail_status: AtomicU16,
    revocations: AtomicUsize,
    historical: Mutex<Option<Value>>,
    list_cutoffs: Mutex<Vec<Option<i64>>>,
}
async fn activity(
    State(vendor): State<Arc<Vendor>>,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    vendor.detail_calls.fetch_add(1, Ordering::SeqCst);
    let status = vendor.detail_status.load(Ordering::SeqCst);
    if status != 0 {
        return (
            StatusCode::from_u16(status).unwrap(),
            Json(json!({"message":"Vendor fixture failure","unmodeled_error":"retained"})),
        );
    }
    let mut value = vendor.activity.lock().unwrap().clone();
    value["id"] = id.into();
    (StatusCode::OK, Json(value))
}
async fn streams(State(vendor): State<Arc<Vendor>>) -> Json<Value> {
    vendor.stream_calls.fetch_add(1, Ordering::SeqCst);
    Json(vendor.streams.lock().unwrap().clone())
}
#[derive(serde::Deserialize)]
struct ListingQuery {
    after: Option<i64>,
}
async fn listing(
    State(vendor): State<Arc<Vendor>>,
    Query(query): Query<ListingQuery>,
) -> Json<Value> {
    vendor.list_calls.fetch_add(1, Ordering::SeqCst);
    vendor.list_cutoffs.lock().unwrap().push(query.after);
    let mut values = vec![vendor.activity.lock().unwrap().clone()];
    values.extend(vendor.historical.lock().unwrap().clone());
    values.retain(|value| {
        let started = chrono::DateTime::parse_from_rfc3339(value["start_date"].as_str().unwrap())
            .unwrap()
            .timestamp();
        query.after.is_none_or(|after| started > after)
    });
    Json(json!(values))
}

async fn revoke(State(vendor): State<Arc<Vendor>>) -> Json<Value> {
    vendor.revocations.fetch_add(1, Ordering::SeqCst);
    Json(json!({"access_token":"fake-access"}))
}

struct Fixture {
    db: DatabaseConnection,
    app: Router,
    vendor: Arc<Vendor>,
    connection: strava_connections::Model,
    base_time: i64,
    _files: tempfile::TempDir,
    server: tokio::task::JoinHandle<()>,
}
impl Fixture {
    async fn new() -> Self {
        let url = std::env::var("BIKE_STRAVA_TEST_DATABASE_URL")
            .expect("A disposable PostgreSQL database is required");
        let db = Database::connect(url).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let files = tempfile::tempdir().unwrap();
        let fixture: Value = serde_json::from_slice(include_bytes!(
            "../../bike-core/testdata/strava-real-ride.json"
        ))
        .unwrap();
        let vendor = Arc::new(Vendor {
            activity: Mutex::new(fixture["activity"].clone()),
            streams: Mutex::new(fixture["streams"].clone()),
            ..Default::default()
        });
        vendor.activity.lock().unwrap()["unmodeled_vendor_field"] = json!({"retained":true});
        let started = (Utc::now() - Duration::days(5)).to_rfc3339();
        vendor.activity.lock().unwrap()["start_date"] = json!(started);
        vendor.activity.lock().unwrap()["start_date_local"] = json!(started);
        let app = Router::new()
            .route("/api/v3/activities/:id", get(activity))
            .route("/api/v3/activities/:id/streams", get(streams))
            .route("/api/v3/athlete/activities", get(listing))
            .route("/oauth/deauthorize", post(revoke))
            .with_state(vendor.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        std::env::set_var(
            "STRAVA_API_ORIGIN",
            format!("http://{}", listener.local_addr().unwrap()),
        );
        std::env::set_var("UPLOADS_DIR", files.path());
        std::env::set_var("APP_ENV", "test");
        std::env::set_var("STRAVA_CLIENT_ID", "fixture");
        std::env::set_var("STRAVA_CLIENT_SECRET", "fixture");
        std::env::set_var("STRAVA_SUBSCRIPTION_ID", "1");
        std::env::set_var("STRAVA_WEBHOOK_VERIFY_TOKEN", "fixture-verify");
        Config::init_from_env();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let user = users::ActiveModel {
            pid: Set(uuid::Uuid::new_v4()),
            email: Set(format!("strava-{}@example.invalid", uuid::Uuid::new_v4())),
            api_key: Set(uuid::Uuid::new_v4().to_string()),
            name: Set("Strava workflow fixture".into()),
            is_admin: Set(Some(true)),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let connection = strava_connections::ActiveModel {
            user_id: Set(user.id),
            athlete_id: Set(35999641),
            scopes: Set("activity:read_all".into()),
            access_token: Set("fake-access".into()),
            refresh_token: Set("fake-refresh".into()),
            expires_at: Set(Utc::now() + Duration::hours(6)),
            last_sync_status: Set("never".into()),
            last_sync_imported_count: Set(0),
            last_sync_duplicate_count: Set(0),
            last_sync_failed_count: Set(0),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let app = webhook_app(db.clone(), files.path().to_str().unwrap(), user.pid);
        Self {
            db,
            app,
            vendor,
            connection,
            base_time: Utc::now().timestamp() - 60,
            _files: files,
            server,
        }
    }

    fn event(&self, action: &str, offset: i64, id: i64) -> StravaWebhookEvent {
        serde_json::from_value(json!({"aspect_type":action,"event_time":self.base_time + offset,"object_id":id,"object_type":"activity","owner_id":35999641,"subscription_id":1,"updates":{}})).unwrap()
    }
    async fn post_webhook(&self, raw: &[u8], expected: StatusCode) {
        let response = self
            .app
            .clone()
            .oneshot(
                Request::post("/webhooks/strava-gateway")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(raw.to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    async fn account_request(&self, method: &str, path: &str) -> Value {
        let response = self
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{method} {path}");
        let body = axum::body::to_bytes(response.into_body(), 1_048_576)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }
    async fn accept(&self, event: &StravaWebhookEvent) -> String {
        let raw = serde_json::to_vec(event).unwrap();
        self.post_webhook(&raw, StatusCode::OK).await;
        let rows = strava_workflows::Entity::find()
            .all(&self.db)
            .await
            .unwrap();
        rows.into_iter()
            .find(|row| {
                row.source["event_time"] == event.event_time
                    && row.action
                        == match event.aspect_type.as_str() {
                            "create" => "insert",
                            "update" if event.object_type == "athlete" => "deauthorize",
                            "update" => "update",
                            _ => "delete",
                        }
            })
            .unwrap()
            .id
    }
    async fn execute(&self, id: &str) {
        let job = background_tasks::Entity::find()
            .filter(background_tasks::Column::TaskType.eq("strava_workflow"))
            .all(&self.db)
            .await
            .unwrap()
            .into_iter()
            .find(|job| job.payload["data"]["workflow_id"] == id)
            .unwrap();
        worker::tasks::processors::StravaWorkflow::new(self.db.clone())
            .process(job.id, job.payload)
            .await
            .unwrap();
    }
    async fn assert_dag_completed(&self, id: &str) {
        let row = strava_workflows::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.status, "completed");
        let store = bike_core::strava_objects::ObjectStore::new(&Config::get().uploads_dir);
        let events = integration_events::Entity::find()
            .filter(integration_events::Column::ConnectionId.eq(row.connection_id))
            .all(&self.db)
            .await
            .unwrap();
        for stage in strava_workflow::graph(&row.action).unwrap() {
            let stage = if stage == "derived" && row.action == "delete" {
                "delete_derived"
            } else {
                stage
            };
            let (_, saved) = row
                .checkpoint
                .as_object()
                .unwrap()
                .iter()
                .find(|(key, _)| key.starts_with(&format!("{stage}:")))
                .unwrap();
            let reference: bike_core::strava_objects::ObjectRef =
                serde_json::from_value(saved.clone()).unwrap();
            assert!(reference
                .path
                .starts_with(&format!("strava/{}/", row.athlete_id)));
            if let Some(activity) = row.activity_id {
                assert!(reference.path.contains(&format!("/{activity}/transforms/")));
            }
            let _: Value = serde_json::from_slice(&store.read(&reference).await.unwrap()).unwrap();
            assert!(
                events
                    .iter()
                    .any(|event| event.payload.as_ref().is_some_and(|payload| payload
                        ["workflow_id"]
                        == id
                        && payload["correlation_id"] == row.correlation_id()
                        && payload["stage"] == stage
                        && payload["athlete_id"] == row.athlete_id
                        && payload["strava_activity_id"] == json!(row.activity_id)
                        && payload["object"] == *saved)),
                "stage {stage} has correlated, retained evidence"
            );
        }
        if let Some(raw) = row.source.get("raw").filter(|raw| !raw.is_null()) {
            let reference = serde_json::from_value(raw.clone()).unwrap();
            assert!(!store.read(&reference).await.unwrap().is_empty());
        }
    }
    async fn local(&self, id: i64) -> Option<activities::Model> {
        activities::Model::strava_activity(&self.db, self.connection.user_id, id)
            .await
            .unwrap()
    }
    fn calls(&self) -> (usize, usize, usize) {
        (
            self.vendor.detail_calls.load(Ordering::SeqCst),
            self.vendor.stream_calls.load(Ordering::SeqCst),
            self.vendor.list_calls.load(Ordering::SeqCst),
        )
    }
    async fn webhook_verification_and_rejections(&self) {
        for path in ["/api/strava/webhook", "/webhooks/strava-gateway"] {
            let response = self.app.clone().oneshot(
                Request::get(format!("{path}?hub.mode=subscribe&hub.challenge=fixture-challenge&hub.verify_token=fixture-verify"))
                    .body(axum::body::Body::empty()).unwrap(),
            ).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let body = axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap(),
                json!({"hub.challenge":"fixture-challenge"})
            );
        }
        self.post_webhook(b"{invalid", StatusCode::BAD_REQUEST)
            .await;
        let mut wrong = self.event("create", 0, 887654322);
        wrong.subscription_id = 2;
        self.post_webhook(
            &serde_json::to_vec(&wrong).unwrap(),
            StatusCode::BAD_REQUEST,
        )
        .await;
        wrong.subscription_id = 1;
        wrong.owner_id = 999;
        self.post_webhook(&serde_json::to_vec(&wrong).unwrap(), StatusCode::OK)
            .await;
        assert_eq!(
            strava_workflows::Entity::find()
                .count(&self.db)
                .await
                .unwrap(),
            0
        );
        assert_eq!(self.calls(), (0, 0, 0));
    }
    async fn insert_and_update(&self) {
        let event = self.event("create", 0, 887654322);
        let id = self.accept(&event).await;
        self.assert_duplicate_webhook(&event).await;
        let (one, two) = tokio::join!(
            strava_workflow::execute(&self.db, &id),
            strava_workflow::execute(&self.db, &id)
        );
        one.unwrap();
        two.unwrap();
        assert_eq!(self.calls(), (1, 1, 0));
        self.assert_dag_completed(&id).await;
        self.assert_metadata_update().await;
    }

    async fn assert_duplicate_webhook(&self, event: &StravaWebhookEvent) {
        let jobs = background_tasks::Entity::find()
            .count(&self.db)
            .await
            .unwrap();
        self.post_webhook(&serde_json::to_vec(event).unwrap(), StatusCode::OK)
            .await;
        assert_eq!(
            strava_workflows::Entity::find()
                .count(&self.db)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            background_tasks::Entity::find()
                .count(&self.db)
                .await
                .unwrap(),
            jobs
        );
    }

    async fn assert_metadata_update(&self) {
        let original = self.local(887654322).await.unwrap();
        assert_eq!(original.title, "Morning Ride fixture");
        let mut model: activities::ActiveModel = original.clone().into();
        model.activity_type = Set("race".into());
        model.update(&self.db).await.unwrap();
        self.vendor.activity.lock().unwrap()["name"] = json!("Updated title");
        let update = self.accept(&self.event("update", 1, 887654322)).await;
        self.execute(&update).await;
        self.assert_dag_completed(&update).await;
        let updated = self.local(887654322).await.unwrap();
        assert_eq!(updated.id, original.id);
        assert_eq!(updated.title, "Updated title");
        assert_eq!(updated.activity_type, "race");
        assert_eq!(
            self.calls(),
            (2, 1, 0),
            "metadata edits reuse stored streams"
        );
    }
    async fn privacy_update_keeps_the_activity(&self) {
        self.vendor.activity.lock().unwrap()["private"] = json!(true);
        let mut event = self.event("update", 2, 887654322);
        event.updates = Some(json!({"private":"true"}));
        let id = self.accept(&event).await;
        let calls = self.calls();
        self.execute(&id).await;
        self.assert_dag_completed(&id).await;
        assert!(self.local(887654322).await.is_some());
        assert_eq!(self.calls(), (calls.0 + 1, calls.1, calls.2));
    }
    async fn sync_and_resume(&self) {
        let updated_at = self.local(887654322).await.unwrap().updated_at;
        for backfill in [false, true] {
            self.sync_collection(backfill).await;
        }
        assert_eq!(self.calls(), (6, 1, 2));
        assert_eq!(
            self.local(887654322).await.unwrap().updated_at,
            updated_at,
            "unchanged sync never rewrites an activity"
        );
    }

    async fn sync_collection(&self, backfill: bool) {
        if !backfill {
            let mut old = self.vendor.activity.lock().unwrap().clone();
            old["id"] = json!(887654999);
            old["start_date"] = json!((Utc::now() - Duration::days(16)).to_rfc3339());
            *self.vendor.historical.lock().unwrap() = Some(old);
        }
        let path = if backfill {
            "/api/strava/backfill"
        } else {
            "/api/strava/sync"
        };
        let response = self.account_request("POST", path).await;
        assert_eq!(response["last_sync_status"], "queued");
        let id = strava_workflows::Model::active_collection(&self.db, self.connection.id)
            .await
            .unwrap()
            .unwrap()
            .id;
        self.assert_queued_collection_visible(&id, backfill).await;
        self.execute(&id).await;
        let row = strava_workflows::Entity::find_by_id(&id)
            .one(&self.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            self.vendor
                .list_cutoffs
                .lock()
                .unwrap()
                .last()
                .copied()
                .unwrap(),
            row.source["after"].as_i64(),
            "the SDK request must carry the accepted history cutoff"
        );
        *self.vendor.historical.lock().unwrap() = None;
        assert!(
            self.local(887654999).await.is_none(),
            "signup must not import older than 15 days"
        );
        self.assert_dag_completed(&id).await;
        assert_eq!(
            self.account_request("GET", "/api/activity-imports/processing-state")
                .await["is_active"],
            false,
            "completed native collections must clear processing state"
        );
        let connection = strava_connections::Entity::find_by_id(self.connection.id)
            .one(&self.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(connection.last_sync_status, "succeeded");
        assert_eq!(connection.last_sync_imported_count, 0);
        assert_eq!(connection.last_sync_duplicate_count, 1);
        self.retry_collection_from_sources(&id).await;
    }

    async fn retry_collection_from_sources(&self, id: &str) {
        let calls = self.calls();
        // Simulate loss of the parent checkpoint after its source objects and
        // children committed. Resume reuses both and makes zero vendor calls.
        let row = strava_workflows::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .unwrap()
            .unwrap();
        let mut retry: strava_workflows::ActiveModel = row.into();
        retry.checkpoint = Set(json!({}));
        retry.status = Set("failed".into());
        retry.update(&self.db).await.unwrap();
        self.execute(id).await;
        assert_eq!(self.calls(), calls);
    }

    async fn assert_queued_collection_visible(&self, id: &str, backfill: bool) {
        let row = strava_workflows::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .unwrap()
            .unwrap();
        if backfill {
            assert!(row.source["after"].is_null());
        } else {
            assert_eq!(
                row.source["after"].as_i64().unwrap(),
                row.source["event_time"].as_i64().unwrap() - 15 * 86_400
            );
        }
        let state = self
            .account_request("GET", "/api/activity-imports/processing-state")
            .await;
        assert_eq!(state["is_active"], true);
        assert_eq!(state["source"], "strava_sync");
    }

    async fn admin_imports_and_integrations(&self) {
        let calls = self.calls();
        let activity = self.local(887654322).await.unwrap();
        let import_id = activity.activity_import_id.unwrap();
        let history = self
            .account_request(
                "GET",
                &format!(
                    "/api/admin/activity-imports/history?user_id={}&source=strava_sync",
                    self.connection.user_id
                ),
            )
            .await;
        assert!(history["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == import_id && item["status"] == "processed"));
        let path = format!("/api/admin/activity-imports/{import_id}/trace");
        let trace = self.account_request("GET", &path).await;
        assert_eq!(trace["import"]["source"], "strava_sync");
        assert!(!trace["attempts"].as_array().unwrap().is_empty());
        assert!(trace["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["status"] == "completed"));
        let events = self
            .account_request("GET", "/api/admin/integration-events?provider=strava")
            .await;
        for kind in [
            "workflow.accepted",
            "workflow.stage",
            "workflow.stage.failed",
        ] {
            assert!(events
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["provider"] == "strava" && event["event_type"] == kind));
        }
        let events = self
            .account_request(
                "GET",
                &format!("/api/admin/integration-events?provider=strava&import_id={import_id}"),
            )
            .await;
        assert!(events
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["payload"]["activity_id"] == activity.id
                && event["payload"]["correlation_id"] == "887654322"));
        self.replay_admin_import(import_id).await;
        let trace = self.account_request("GET", &path).await;
        assert_eq!(trace["attempts"][0]["status"], "completed");
        assert_eq!(
            self.calls(),
            calls,
            "admin inspection and replay reuse retained Strava data"
        );
    }

    async fn replay_admin_import(&self, import_id: i32) {
        let path = format!("/api/admin/activity-imports/{import_id}/replay");
        let plan = self
            .account_request("GET", &format!("{path}?stage=activity_parsed"))
            .await;
        let response = self.app.clone().oneshot(Request::builder().method("POST").uri(path)
            .header("content-type", "application/json")
            .body(axum::body::Body::from(json!({"stage":"activity_parsed", "expected_start_stage":plan["start_stage"], "expected_reused_attempt_id":plan["reused_attempt_id"]}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let job = background_tasks::Entity::find()
            .filter(background_tasks::Column::TaskType.eq("process_activity_import"))
            .all(&self.db)
            .await
            .unwrap()
            .into_iter()
            .find(|job| job.payload["data"]["import_id"] == import_id)
            .unwrap();
        worker::tasks::processors::ProcessActivityImport::new(self.db.clone())
            .process(job.id, job.payload)
            .await
            .unwrap();
    }
    async fn delete_and_reject_stale_work(&self) {
        let offset = Utc::now().timestamp() - self.base_time + 4;
        let first = self.accept(&self.event("delete", offset, 887654322)).await;
        self.execute(&first).await;
        self.assert_dag_completed(&first).await;
        assert!(self.local(887654322).await.is_none());
        self.retry_after_derived_intent_commit(&first).await;
        let second = self
            .accept(&self.event("delete", offset + 1, 887654322))
            .await;
        self.execute(&second).await;
        self.assert_dag_completed(&second).await;
        let row = strava_workflows::Entity::find_by_id(second)
            .one(&self.db)
            .await
            .unwrap()
            .unwrap();
        let object = serde_json::from_value(
            row.checkpoint
                .as_object()
                .unwrap()
                .iter()
                .find(|(key, _)| key.starts_with("delete:"))
                .unwrap()
                .1
                .clone(),
        )
        .unwrap();
        let bytes = bike_core::strava_objects::ObjectStore::new(&Config::get().uploads_dir)
            .read(&object)
            .await
            .unwrap();
        let effect: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(effect["outcome"], "already_absent");
        let calls = self.calls();
        self.execute(&self.accept(&self.event("update", -1, 887654322)).await)
            .await;
        assert!(self.local(887654322).await.is_none());
        assert_eq!(
            self.calls(),
            calls,
            "stale updates do not refetch or resurrect deleted activities"
        );
    }

    async fn failed_reads_preserve_the_activity(&self) {
        let before = self.local(887654322).await.unwrap();
        let mut last = String::new();
        for (offset, status) in [(3, 503), (4, 404)] {
            self.vendor.detail_status.store(status, Ordering::SeqCst);
            last = self.accept(&self.event("update", offset, 887654322)).await;
            assert!(strava_workflow::execute(&self.db, &last).await.is_err());
            let after = self.local(887654322).await.unwrap();
            assert_eq!(after.id, before.id);
            assert_eq!(
                after.updated_at, before.updated_at,
                "HTTP failures never become deletes or applied updates"
            );
            let row = strava_workflows::Entity::find_by_id(&last)
                .one(&self.db)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.status, "failed");
            assert!(row.error.is_some());
        }
        self.vendor.detail_status.store(0, Ordering::SeqCst);
        self.execute(&last).await;
        assert_eq!(
            self.local(887654322).await.unwrap().updated_at,
            before.updated_at
        );
        assert_eq!(
            self.calls(),
            (6, 1, 0),
            "failed responses are retained as attempts and a successful retry fetches once"
        );
    }

    async fn quota_pause_and_resume(&self) {
        use bike_core::entities::provider_rate_limit_buckets as quota;
        let bucket = quota::Model::find_bucket(&self.db, "strava", "read_15_minute")
            .await
            .unwrap()
            .unwrap();
        let mut exhausted: quota::ActiveModel = bucket.clone().into();
        exhausted.used_count = Set(bucket.limit_count);
        exhausted.reset_at = Set(Utc::now() + Duration::minutes(5));
        exhausted.update(&self.db).await.unwrap();
        let offset = Utc::now().timestamp() - self.base_time + 1;
        let id = self.accept(&self.event("update", offset, 887654322)).await;
        let calls = self.calls();
        let error = strava_workflow::execute(&self.db, &id).await.unwrap_err();
        assert_eq!(error.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            self.calls(),
            calls,
            "exhausted quota makes no vendor request"
        );
        strava_workflow::resume_at(&self.db, &id, error.retry_at.unwrap())
            .await
            .unwrap();
        self.execute(&id).await;
        assert_eq!(
            self.calls(),
            calls,
            "a paused worker makes no vendor request"
        );
        self.assert_pause_visible(&id).await;
        let mut reset: quota::ActiveModel = bucket.into();
        reset.reset_at = Set(Utc::now() - Duration::seconds(1));
        reset.update(&self.db).await.unwrap();
        self.execute(&id).await;
        assert_eq!(self.calls(), (calls.0 + 1, calls.1, calls.2));
        assert!(
            strava_workflows::Model::waiting(&self.db, self.connection.id)
                .await
                .unwrap()
                .is_none()
        );
        let after = self.calls();
        self.execute(&id).await;
        assert_eq!(
            self.calls(),
            after,
            "completed workflow redelivery consumes no quota"
        );
        self.completed_redelivery_while_paused(&id).await;
    }

    async fn completed_redelivery_while_paused(&self, id: &str) {
        let jobs = background_tasks::Entity::find()
            .count(&self.db)
            .await
            .unwrap();
        let calls = self.calls();
        std::env::set_var("STRAVA_PROVIDER_WORK_ENABLED", "false");
        self.execute(id).await;
        std::env::set_var("STRAVA_PROVIDER_WORK_ENABLED", "true");
        assert_eq!(self.calls(), calls);
        assert_eq!(
            background_tasks::Entity::find()
                .count(&self.db)
                .await
                .unwrap(),
            jobs
        );
        self.assert_dag_completed(id).await;
    }

    async fn assert_pause_visible(&self, id: &str) {
        let row = strava_workflows::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.status, "accepted");
        assert_eq!(row.checkpoint["pause"]["reason"], "rate_limit");
        assert!(
            strava_workflows::Model::waiting(&self.db, self.connection.id)
                .await
                .unwrap()
                .is_some()
        );
        let response = self.account_request("GET", "/api/strava/connection").await;
        assert_eq!(response["last_sync_wait_reason"], "rate_limit");
        assert!(response["last_sync_next_attempt_at"].is_string());
    }

    async fn disconnect_is_idempotent(&self) {
        let offset = Utc::now().timestamp() - self.base_time + 5;
        let id = self.accept(&self.event("create", offset, 887654323)).await;
        self.execute(&id).await;
        let pending = self
            .accept(&self.event("update", offset + 1, 887654323))
            .await;
        self.account_request("DELETE", "/api/strava/connection")
            .await;
        let disconnect = strava_workflows::Model::pending_revocation(&self.db, &self.connection)
            .await
            .unwrap()
            .unwrap()
            .id;
        assert_eq!(
            strava_workflow::queue_disconnect(&self.db, self.connection.user_id)
                .await
                .unwrap(),
            Some(disconnect.clone())
        );
        let calls = self.calls();
        self.execute(&pending).await;
        assert_eq!(
            self.calls(),
            calls,
            "pending revocation prevents another activity fetch"
        );
        self.execute(&disconnect).await;
        self.assert_dag_completed(&disconnect).await;
        self.execute(&disconnect).await;
        assert_eq!(self.vendor.revocations.load(Ordering::SeqCst), 1);
        assert!(self.local(887654323).await.is_none());
        assert!(strava_connections::Entity::find_by_id(self.connection.id)
            .one(&self.db)
            .await
            .unwrap()
            .is_none());
    }
    async fn sport_update_removes_only_the_cycling_activity(&self) {
        let offset = Utc::now().timestamp() - self.base_time + 12;
        let insert = self.accept(&self.event("create", offset, 887654325)).await;
        self.execute(&insert).await;
        assert!(self.local(887654325).await.is_some());
        {
            let mut activity = self.vendor.activity.lock().unwrap();
            activity["type"] = json!("Run");
            activity["sport_type"] = json!("Run");
        }
        let update = self
            .accept(&self.event("update", offset + 1, 887654325))
            .await;
        let calls = self.calls();
        self.execute(&update).await;
        self.assert_dag_completed(&update).await;
        assert!(self.local(887654325).await.is_none());
        assert_eq!(self.calls(), (calls.0 + 1, calls.1, calls.2));
        let mut activity = self.vendor.activity.lock().unwrap();
        activity["type"] = json!("Ride");
        activity["sport_type"] = json!("Ride");
    }
    async fn vendor_revocation_uses_local_cleanup(&self) {
        let mut reconnect: strava_connections::ActiveModel = self.connection.clone().into();
        reconnect.id = sea_orm::ActiveValue::NotSet;
        reconnect.created_at = Set(Utc::now());
        let connection = reconnect.insert(&self.db).await.unwrap();
        let offset = Utc::now().timestamp() - self.base_time + 20;
        let create = self.accept(&self.event("create", offset, 887654324)).await;
        self.execute(&create).await;
        let mut event = self.event("update", offset + 1, self.connection.athlete_id);
        event.object_type = "athlete".into();
        event.updates = Some(json!({"authorized":"false"}));
        let id = self.accept(&event).await;
        let calls = self.calls();
        let revocations = self.vendor.revocations.load(Ordering::SeqCst);
        self.execute(&id).await;
        self.assert_dag_completed(&id).await;
        self.post_webhook(&serde_json::to_vec(&event).unwrap(), StatusCode::OK)
            .await;
        self.execute(&id).await;
        assert!(self.local(887654324).await.is_none());
        assert!(strava_connections::Entity::find_by_id(connection.id)
            .one(&self.db)
            .await
            .unwrap()
            .is_none());
        assert_eq!(self.calls(), calls);
        assert_eq!(self.vendor.revocations.load(Ordering::SeqCst), revocations);
    }

    async fn retry_after_derived_intent_commit(&self, id: &str) {
        let count = bike_core::background_jobs::background_tasks::Entity::find()
            .count(&self.db)
            .await
            .unwrap();
        let row = strava_workflows::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .unwrap()
            .unwrap();
        let mut checkpoint = row.checkpoint.clone();
        let saved = checkpoint
            .as_object()
            .unwrap()
            .iter()
            .find(|(key, _)| key.starts_with("delete_derived:"))
            .unwrap()
            .1
            .clone();
        let object: bike_core::strava_objects::ObjectRef = serde_json::from_value(saved).unwrap();
        let directory = std::path::Path::new(&Config::get().uploads_dir).join(&object.path);
        tokio::fs::remove_file(directory.parent().unwrap().join("reference.json"))
            .await
            .unwrap();
        assert!(
            checkpoint
                .as_object()
                .unwrap()
                .keys()
                .any(|key| key.starts_with("job:")),
            "stage completion preserves queued-job checkpoints"
        );
        checkpoint
            .as_object_mut()
            .unwrap()
            .retain(|key, _| !key.starts_with("delete_derived:") && !key.starts_with("derived:"));
        let mut retry: strava_workflows::ActiveModel = row.into();
        retry.checkpoint = Set(checkpoint);
        retry.status = Set("failed".into());
        retry.update(&self.db).await.unwrap();
        self.execute(id).await;
        assert_eq!(
            bike_core::background_jobs::background_tasks::Entity::find()
                .count(&self.db)
                .await
                .unwrap(),
            count,
            "an ambiguous derived-stage retry never duplicates its queued jobs"
        );
    }
}

fn webhook_app(db: DatabaseConnection, uploads_dir: &str, user_pid: uuid::Uuid) -> Router {
    let state = Arc::new(api::storage::AppStorage {
        map_images: None,
        heatmaps: Arc::new(bike_core::heatmaps::service::HeatmapService::default()),
        tasks: api::tasks::TaskQueue::new(db.clone()),
        feature_flags: bike_core::platform::feature_flags::FeatureFlagService::new(),
        session_service: api::tasks::create_session_service(db.clone()),
        db,
        uploads_dir: uploads_dir.into(),
        local_admin_user_pid: Some(user_pid),
        synthetic_auth: None,
    });
    api::controllers::routes().with_state(state)
}

#[tokio::test]
#[ignore = "Requires BIKE_STRAVA_TEST_DATABASE_URL; run mise run test:strava:postgres"]
async fn native_actions_reuse_sources_and_apply_once() {
    let fixture = Box::pin(Fixture::new()).await;
    Box::pin(fixture.webhook_verification_and_rejections()).await;
    for action in ["insert", "update", "upsert", "delete", "sync", "backfill"] {
        assert!(!strava_workflow::graph(action).unwrap().is_empty());
    }
    Box::pin(fixture.insert_and_update()).await;
    Box::pin(fixture.privacy_update_keeps_the_activity()).await;
    Box::pin(fixture.failed_reads_preserve_the_activity()).await;
    Box::pin(fixture.admin_imports_and_integrations()).await;
    Box::pin(fixture.sync_and_resume()).await;
    Box::pin(fixture.quota_pause_and_resume()).await;
    Box::pin(fixture.delete_and_reject_stale_work()).await;
    Box::pin(fixture.sport_update_removes_only_the_cycling_activity()).await;
    Box::pin(fixture.disconnect_is_idempotent()).await;
    Box::pin(fixture.vendor_revocation_uses_local_cleanup()).await;
    fixture.server.abort();
}
