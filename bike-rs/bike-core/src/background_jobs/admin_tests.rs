use super::{
    admin::{self, AdminVerified, BackgroundTasksStorage},
    background_tasks,
    history_tests::{database, enqueue},
    pipeline::PipelineContext,
};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::json;
use std::sync::Arc;

struct FixtureStorage(DatabaseConnection);
impl BackgroundTasksStorage for FixtureStorage {
    fn db(&self) -> &DatabaseConnection {
        &self.0
    }
}
struct TestAdmin;
impl AdminVerified for TestAdmin {}

fn state(db: &DatabaseConnection) -> State<Arc<FixtureStorage>> {
    State(Arc::new(FixtureStorage(db.clone())))
}

#[tokio::test]
async fn admin_search_and_detail_follow_a_request_into_child_history() {
    let db = database(true).await;
    let root = PipelineContext::received("upload", Some("admin-search-request".into()));
    let parent = PipelineContext::scope(Some(root.clone()), enqueue(&db)).await;
    let child = PipelineContext::scope(Some(root.clone().for_task(parent.id)), enqueue(&db)).await;
    child
        .claim(&db)
        .await
        .unwrap()
        .unwrap()
        .finish_execution(&db, Ok(()))
        .await
        .unwrap();
    let query = serde_json::from_value(json!({"correlation_id": "admin-search-request"})).unwrap();
    let page = admin::list_tasks(TestAdmin, state(&db), Query(query))
        .await
        .unwrap()
        .0;
    assert_eq!(page.metadata.total, 2);
    let detail = admin::get_task(TestAdmin, state(&db), Path(child.id))
        .await
        .unwrap()
        .0;
    assert_eq!(detail.pipelines[0].run_id, root.run_id);
    assert_eq!(detail.pipelines[0].parent_task_id, Some(parent.id));
    assert_eq!(detail.attempt_history[0].outcome, "completed");
    assert!(detail.pipelines[0].available_at.is_none());
}

#[tokio::test]
async fn admin_rerun_uses_a_new_origin_and_import_attempt() {
    let db = database(true).await;
    let original = PipelineContext::received("upload", Some("old-request".into()));
    let task = PipelineContext::scope(
        Some(original.clone()),
        background_tasks::Model::enqueue(
            &db,
            "process_activity_import".into(),
            json!({"data": {"import_id": 8, "attempt_id": 99}}),
            None,
            3,
        ),
    )
    .await
    .unwrap();
    let request = PipelineContext::received("api", Some("new-request".into()));
    let rerun = PipelineContext::scope(
        Some(request),
        admin::rerun_task(TestAdmin, state(&db), Path(task.id)),
    )
    .await
    .unwrap()
    .0;
    let created = background_tasks::Entity::find_by_id(rerun.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let origin = PipelineContext::from_payload(&created.payload)
        .unwrap()
        .unwrap();
    assert_ne!(origin.run_id, original.run_id);
    assert_eq!(origin.entrypoint, "admin_rerun");
    assert_eq!(origin.request_id.as_deref(), Some("new-request"));
    assert!(origin.parent_task_id.is_none());
    assert!(created.payload["data"].get("attempt_id").is_none());
    assert_eq!(created.payload["data"]["import_id"], 8);
}

#[tokio::test]
async fn admin_cancel_records_history_and_rejects_unclaimable_tasks() {
    let db = database(true).await;
    let pending = enqueue(&db).await;
    let invalid = admin::cancel_task(TestAdmin, state(&db), Path(pending.id))
        .await
        .unwrap_err()
        .into_response();
    assert_eq!(invalid.status(), 400);
    let active = pending.claim(&db).await.unwrap().unwrap();
    let canceled = admin::cancel_task(TestAdmin, state(&db), Path(active.id))
        .await
        .unwrap()
        .0;
    assert_eq!(canceled.status, "canceled");
    assert!(active
        .finish_execution(&db, Ok(()))
        .await
        .unwrap()
        .is_none());
    let detail = admin::get_task(TestAdmin, state(&db), Path(active.id))
        .await
        .unwrap()
        .0;
    assert_eq!(detail.attempt_history[0].outcome, "canceled");
    let absent = admin::get_task(TestAdmin, state(&db), Path(i32::MAX))
        .await
        .unwrap_err()
        .into_response();
    assert_eq!(absent.status(), 404);
}
