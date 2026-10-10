use super::{
    background_tasks,
    entities::{pipeline_runs, pipeline_tasks, task_attempts},
    pipeline::PipelineContext,
};
use chrono::{Duration, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Schema,
};
use serde_json::json;

pub(crate) async fn database(with_history: bool) -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(db.get_database_backend());
    db.execute(&schema.create_table_from_entity(background_tasks::Entity))
        .await
        .unwrap();
    if with_history {
        db.execute(&schema.create_table_from_entity(pipeline_runs::Entity))
            .await
            .unwrap();
        db.execute(&schema.create_table_from_entity(pipeline_tasks::Entity))
            .await
            .unwrap();
        db.execute(&schema.create_table_from_entity(task_attempts::Entity))
            .await
            .unwrap();
        for statement in [
            schema.create_table_from_entity(super::entities::processor_registry::Entity),
            schema.create_table_from_entity(super::entities::work_units::Entity),
            schema.create_table_from_entity(super::entities::pipeline_subjects::Entity),
            schema.create_table_from_entity(super::entities::pipeline_outputs::Entity),
            schema.create_table_from_entity(super::entities::task_anomalies::Entity),
            schema.create_table_from_entity(super::entities::worker_batches::Entity),
            schema.create_table_from_entity(super::entities::batch_tasks::Entity),
            schema.create_table_from_entity(crate::entities::activity_import_attempts::Entity),
            schema.create_table_from_entity(crate::entities::activity_imports::Entity),
            schema.create_table_from_entity(crate::entities::activity_import_locks::Entity),
        ] {
            db.execute(&statement).await.unwrap();
        }
    }
    db
}

pub(crate) async fn diagnostic_tables(db: &DatabaseConnection) {
    let schema = Schema::new(db.get_database_backend());
    for mut statement in [
        schema.create_table_from_entity(pipeline_runs::Entity),
        schema.create_table_from_entity(pipeline_tasks::Entity),
        schema.create_table_from_entity(task_attempts::Entity),
        schema.create_table_from_entity(super::entities::pipeline_outputs::Entity),
        schema.create_table_from_entity(super::entities::pipeline_subjects::Entity),
        schema.create_table_from_entity(super::entities::work_units::Entity),
        schema.create_table_from_entity(super::entities::task_anomalies::Entity),
        schema.create_table_from_entity(super::entities::processor_registry::Entity),
        schema.create_table_from_entity(super::entities::worker_batches::Entity),
        schema.create_table_from_entity(super::entities::batch_tasks::Entity),
    ] {
        statement.if_not_exists();
        db.execute(&statement).await.unwrap();
    }
}

pub(super) async fn enqueue(db: &DatabaseConnection) -> background_tasks::Model {
    background_tasks::Model::enqueue(
        db,
        "rebuild_fitness_freshness".into(),
        json!({"data": {"user_id": 7}}),
        None,
        2,
    )
    .await
    .unwrap()
}

async fn attempts(db: &DatabaseConnection, id: i32) -> Vec<task_attempts::Model> {
    task_attempts::Entity::find()
        .filter(task_attempts::Column::TaskId.eq(id))
        .order_by_asc(task_attempts::Column::Attempt)
        .all(db)
        .await
        .unwrap()
}

#[tokio::test]
async fn retry_history_preserves_both_executions_and_fences_stale_results() {
    let db = database(true).await;
    let pending = enqueue(&db).await;
    background_tasks::Entity::update_many()
        .col_expr(
            background_tasks::Column::ScheduledFor,
            sea_orm::sea_query::Expr::value(Utc::now() - Duration::days(1)),
        )
        .filter(background_tasks::Column::Id.eq(pending.id))
        .exec(&db)
        .await
        .unwrap();
    let first = pending.claim(&db).await.unwrap().unwrap();
    assert!(pending.claim(&db).await.unwrap().is_none());
    let retry = first
        .mark_failed(&db, "temporary provider failure".into())
        .await
        .unwrap();
    assert_eq!(retry.status, "pending");
    let second = retry.claim(&db).await.unwrap().unwrap();
    assert!(!first.heartbeat(&db).await.unwrap());
    assert_eq!(
        first.mark_completed(&db).await.unwrap().status,
        "processing"
    );
    let done = second.mark_completed(&db).await.unwrap();
    assert_eq!(done.status, "completed");
    let history = attempts(&db, pending.id).await;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].outcome, "retrying");
    assert_eq!(
        history[0].error.as_deref(),
        Some("temporary provider failure")
    );
    assert_eq!(history[1].outcome, "completed");
    assert_eq!(history[1].eligible_at, Some(retry.updated_at));
    assert!(history.iter().all(|attempt| attempt.finished_at.is_some()));
}

#[tokio::test]
async fn canceled_execution_cannot_complete_and_heartbeat_is_not_progress() {
    let db = database(true).await;
    let active = enqueue(&db).await.claim(&db).await.unwrap().unwrap();
    let initial = attempts(&db, active.id).await.remove(0);
    assert!(active.heartbeat(&db).await.unwrap());
    let heartbeat = attempts(&db, active.id).await.remove(0);
    assert_eq!(initial.progress_at, heartbeat.progress_at);
    active.cancel(&db).await.unwrap();
    assert!(active
        .finish_execution(&db, Ok(()))
        .await
        .unwrap()
        .is_none());
    assert_eq!(active.mark_completed(&db).await.unwrap().status, "canceled");
    assert!(!active.heartbeat(&db).await.unwrap());
    assert_eq!(attempts(&db, active.id).await[0].outcome, "canceled");
}

#[tokio::test]
async fn exhausted_retries_and_future_tasks_are_not_claimable() {
    let db = database(true).await;
    let task = enqueue(&db).await;
    let first = task.claim(&db).await.unwrap().unwrap();
    let retry = first.mark_failed(&db, "first".into()).await.unwrap();
    let second = retry.claim(&db).await.unwrap().unwrap();
    let failed = second.mark_failed(&db, "second".into()).await.unwrap();
    assert_eq!(failed.status, "failed");
    assert!(failed.completed_at.is_some());
    assert!(failed.claim(&db).await.unwrap().is_none());
    assert_eq!(attempts(&db, task.id).await[1].outcome, "failed");
    let future = background_tasks::Model::enqueue(
        &db,
        "future".into(),
        json!({}),
        Some(Utc::now() + Duration::hours(1)),
        3,
    )
    .await
    .unwrap();
    assert!(future.claim(&db).await.unwrap().is_none());
}

#[tokio::test]
async fn child_enqueue_keeps_original_receipt_and_acceptance() {
    let db = database(true).await;
    let mut origin = PipelineContext::received("upload", Some("request-123".into()));
    origin.pipeline_started_at -= Duration::minutes(2);
    let parent = PipelineContext::scope(Some(origin.clone()), enqueue(&db)).await;
    let root = pipeline_runs::Entity::find_by_id(origin.run_id.clone())
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let child =
        PipelineContext::scope(Some(origin.clone().for_task(parent.id)), enqueue(&db)).await;
    let later = pipeline_runs::Entity::find_by_id(origin.run_id.clone())
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(root, later);
    assert_eq!(root.pipeline_started_at, origin.pipeline_started_at);
    assert!(root.accepted_at > root.pipeline_started_at);
    assert!(root.available_at.is_none());
    let link = pipeline_tasks::Entity::find_by_id((origin.run_id, child.id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(link.parent_task_id, Some(parent.id));
}

#[tokio::test]
async fn enqueue_rolls_back_when_causal_history_cannot_be_written() {
    let db = database(false).await;
    let result = PipelineContext::scope(
        Some(PipelineContext::received("upload", None)),
        background_tasks::Model::enqueue(&db, "test".into(), json!({}), None, 3),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        background_tasks::Entity::find().count(&db).await.unwrap(),
        0
    );
}

#[tokio::test]
async fn failed_attempt_record_rolls_back_the_claim_and_legacy_receipt_stays_unknown() {
    let db = database(false).await;
    let pending = enqueue(&db).await;
    assert!(pending.claim(&db).await.is_err());
    assert_eq!(
        background_tasks::Entity::find_by_id(pending.id)
            .one(&db)
            .await
            .unwrap(),
        Some(pending)
    );
    let db = database(true).await;
    let task = enqueue(&db).await;
    task.claim(&db)
        .await
        .unwrap()
        .unwrap()
        .mark_completed(&db)
        .await
        .unwrap();
    let (attempts, pipelines) = super::history::load(&db, task.id).await.unwrap();
    assert_eq!(attempts.len(), 1);
    assert!(pipelines.is_empty());
}

#[tokio::test]
async fn recovery_preserves_attempts_and_rejects_a_snapshot_before_a_new_heartbeat() {
    let db = database(true).await;
    let first = enqueue(&db).await.claim(&db).await.unwrap().unwrap();
    first.heartbeat(&db).await.unwrap();
    assert!(!first.recover(&db, first.payload.clone()).await.unwrap());
    let current = background_tasks::Entity::find_by_id(first.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(current.recover(&db, current.payload.clone()).await.unwrap());
    let pending = background_tasks::Entity::find_by_id(first.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.attempts, 1);
    assert!(first.finish_execution(&db, Ok(())).await.unwrap().is_none());
    let second = pending.claim(&db).await.unwrap().unwrap();
    assert_eq!(second.attempts, 2);
    assert_eq!(first.mark_completed(&db).await.unwrap().attempts, 2);
    second.mark_completed(&db).await.unwrap();
    let history = attempts(&db, first.id).await;
    assert_eq!(history[0].outcome, "interrupted");
    assert_eq!(history[1].outcome, "completed");
}

#[tokio::test]
async fn admin_history_connects_the_root_parent_and_finished_attempt() {
    let db = database(true).await;
    let root = PipelineContext::received("upload", Some("history-request".into()));
    let parent = PipelineContext::scope(Some(root.clone()), enqueue(&db)).await;
    let child = PipelineContext::scope(Some(root.clone().for_task(parent.id)), enqueue(&db)).await;
    child
        .claim(&db)
        .await
        .unwrap()
        .unwrap()
        .finish_execution(&db, Ok(()))
        .await
        .unwrap()
        .unwrap();
    let (history, pipelines) = super::history::load(&db, child.id).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].outcome, "completed");
    assert!(history[0].finished_at.is_some());
    assert_eq!(pipelines[0].run_id, root.run_id);
    assert_eq!(pipelines[0].parent_task_id, Some(parent.id));
    assert_eq!(pipelines[0].request_id, root.request_id);
    assert!(pipelines[0].available_at.is_none());
    let related =
        background_tasks::Model::with_correlation(background_tasks::Entity::find(), &root.run_id)
            .all(&db)
            .await
            .unwrap();
    assert_eq!(related.len(), 2);
    assert!(background_tasks::Model::with_correlation(
        background_tasks::Entity::find(),
        "missing-request"
    )
    .all(&db)
    .await
    .unwrap()
    .is_empty());
}
