use super::*;
use crate::background_jobs::{entities::task_attempts, history_tests::database};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

struct FixtureProcessor {
    kind: &'static str,
    calls: AtomicUsize,
}

#[async_trait]
impl TaskProcessor for FixtureProcessor {
    fn task_type(&self) -> &str {
        self.kind
    }
    async fn process(&self, _: i32, _: serde_json::Value) -> Result<(), WorkerError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

async fn abandoned(db: &DatabaseConnection, kind: &str) -> background_tasks::Model {
    let origin = PipelineContext::received("upload", Some("crash-request".into()));
    let payload = json!({"data":{"activities":[{"activity_id":42,"generation":1}]}});
    let task = PipelineContext::scope(
        Some(origin),
        background_tasks::Model::enqueue(db, kind.into(), payload, None, 1),
    )
    .await
    .unwrap()
    .claim(db)
    .await
    .unwrap()
    .unwrap();
    expire_heartbeat(db, task.id).await;
    task
}

async fn expire_heartbeat(db: &DatabaseConnection, task_id: i32) {
    let stale = Utc::now() - chrono::Duration::minutes(10);
    background_tasks::Entity::update_many()
        .col_expr(background_tasks::Column::UpdatedAt, Expr::value(stale))
        .filter(background_tasks::Column::Id.eq(task_id))
        .exec(db)
        .await
        .unwrap();
    task_attempts::Entity::update_many()
        .col_expr(task_attempts::Column::HeartbeatAt, Expr::value(stale))
        .filter(task_attempts::Column::TaskId.eq(task_id))
        .exec(db)
        .await
        .unwrap();
}

#[tokio::test]
async fn poll_recovers_abandoned_import_and_heatmap_with_the_original_payload_and_origin() {
    for kind in ["process_activity_import", "prepare_heatmap"] {
        let db = database(true).await;
        let crashed = abandoned(&db, kind).await;
        let processor = Arc::new(FixtureProcessor {
            kind,
            calls: AtomicUsize::new(0),
        });
        let worker = TaskWorker::new(db.clone()).register_processor(processor.clone());
        assert_eq!(worker.process_batch().await.unwrap(), 1);
        let finished = background_tasks::Entity::find_by_id(crashed.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(finished.status, "completed");
        assert_eq!(finished.payload, crashed.payload);
        assert_eq!(finished.attempts, 2);
        assert_eq!(processor.calls.load(Ordering::SeqCst), 1);
        let history = task_attempts::Model::for_task(&db, crashed.id)
            .await
            .unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].outcome, "interrupted");
        assert_eq!(history[1].outcome, "completed");
        assert!(crashed
            .finish_execution(&db, Ok(()))
            .await
            .unwrap()
            .is_none());
    }
}

#[tokio::test]
async fn poll_does_not_reclaim_an_execution_with_a_live_heartbeat() {
    let db = database(true).await;
    let crashed = abandoned(&db, "prepare_heatmap").await;
    assert!(crashed.heartbeat(&db).await.unwrap());
    let processor = Arc::new(FixtureProcessor {
        kind: "prepare_heatmap",
        calls: AtomicUsize::new(0),
    });
    let worker = TaskWorker::new(db.clone()).register_processor(processor.clone());
    assert_eq!(worker.process_batch().await.unwrap(), 0);
    assert_eq!(processor.calls.load(Ordering::SeqCst), 0);
    let active = background_tasks::Entity::find_by_id(crashed.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.status, "processing");
    assert_eq!(active.attempts, 1);
    assert_eq!(
        task_attempts::Model::for_task(&db, crashed.id)
            .await
            .unwrap()[0]
            .outcome,
        "running"
    );
}

#[tokio::test]
async fn poll_never_recovers_canceled_or_exhausted_failed_tasks() {
    for status in ["canceled", "failed"] {
        let db = database(true).await;
        let task = abandoned(&db, "prepare_heatmap").await;
        if status == "canceled" {
            task.cancel(&db).await.unwrap();
        } else {
            task.mark_failed(&db, "exhausted".into()).await.unwrap();
        }
        expire_heartbeat(&db, task.id).await;
        let worker = TaskWorker::new(db.clone());
        assert_eq!(worker.process_batch().await.unwrap(), 0);
        let terminal = background_tasks::Entity::find_by_id(task.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(terminal.status, status);
        assert_eq!(terminal.attempts, 1);
        assert_eq!(terminal.max_attempts, 1);
        let history = task_attempts::Model::for_task(&db, task.id).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].outcome, status);
    }
}
