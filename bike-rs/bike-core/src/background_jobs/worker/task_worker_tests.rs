use super::*;
use crate::background_jobs::{
    entities::task_attempts,
    history_tests::{database, enqueue},
};
use async_trait::async_trait;
use opentelemetry::{global, trace::TracerProvider as _};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{InMemorySpanExporter, SdkTracerProvider},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing_subscriber::prelude::*;

struct HandoffProcessor {
    db: DatabaseConnection,
    calls: AtomicUsize,
}

struct EnqueueStartupHook;

#[async_trait]
impl WorkerStartupHook for EnqueueStartupHook {
    fn name(&self) -> &str {
        "fixture_recovery"
    }
    async fn run(&self, db: &DatabaseConnection) -> Result<(), WorkerError> {
        enqueue(db).await;
        Ok(())
    }
}

#[tokio::test]
async fn startup_recovery_enqueues_a_distinct_maintenance_origin() {
    let db = database(true).await;
    TaskWorker::new(db.clone())
        .register_startup_hook(Arc::new(EnqueueStartupHook))
        .run_startup_hooks()
        .await;
    let task = background_tasks::Entity::find()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let origin = PipelineContext::from_payload(&task.payload)
        .unwrap()
        .unwrap();
    assert_eq!(origin.entrypoint, "worker_startup");
    assert!(origin.parent_task_id.is_none());
    assert!(origin.pipeline_started_at <= task.created_at);
}

#[async_trait]
impl TaskProcessor for HandoffProcessor {
    fn task_type(&self) -> &str {
        "rebuild_fitness_freshness"
    }

    async fn process(&self, _: i32, _: serde_json::Value) -> Result<(), WorkerError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err("fixture retry".into());
        }
        background_tasks::Model::enqueue(&self.db, "downstream_fixture".into(), json!({}), None, 3)
            .await?;
        Ok(())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn worker_restores_trace_and_pipeline_across_retry_and_handoff() {
    global::set_text_map_propagator(TraceContextPropagator::new());
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("worker-test")));
    let _subscriber = tracing::subscriber::set_default(subscriber);
    let db = database(true).await;
    let mut origin = PipelineContext::received("upload", Some("request-handoff".into()));
    let trace_id = "01010101010101010101010101010101";
    origin.trace_context = Some(HashMap::from([(
        "traceparent".into(),
        format!("00-{trace_id}-0202020202020202-01"),
    )]));
    let task = PipelineContext::scope(Some(origin.clone()), enqueue(&db)).await;
    let processor = Arc::new(HandoffProcessor {
        db: db.clone(),
        calls: AtomicUsize::new(0),
    });
    let metrics = Arc::new(WorkerMetrics::new("worker_history_test"));
    let worker = TaskWorker::new(db.clone())
        .register_processor(processor.clone())
        .with_metrics(metrics.clone());
    assert!(worker.process_task(task.clone()).await.unwrap());
    assert!(!worker.process_task(task.clone()).await.unwrap());
    let retry = background_tasks::Entity::find_by_id(task.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(worker.process_task(retry).await.unwrap());
    assert_eq!(processor.calls.load(Ordering::SeqCst), 2);
    assert_attempt_metrics(&metrics).await;
    let history = task_attempts::Model::for_task(&db, task.id).await.unwrap();
    assert_eq!(history.len(), 2);
    assert!(history
        .iter()
        .all(|attempt| attempt.trace_id.as_deref() == Some(trace_id)));
    assert_ne!(history[0].span_id, history[1].span_id);
    let child = background_tasks::Entity::find()
        .filter(background_tasks::Column::TaskType.eq("downstream_fixture"))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let context = PipelineContext::from_payload(&child.payload)
        .unwrap()
        .unwrap();
    assert_eq!(context.run_id, origin.run_id);
    assert_eq!(context.pipeline_started_at, origin.pipeline_started_at);
    assert_eq!(context.parent_task_id, Some(task.id));
    let related = background_tasks::Model::with_correlation(
        background_tasks::Entity::find(),
        "request-handoff",
    )
    .all(&db)
    .await
    .unwrap();
    assert_eq!(related.len(), 2);
    let traced =
        background_tasks::Model::with_correlation(background_tasks::Entity::find(), trace_id)
            .all(&db)
            .await
            .unwrap();
    assert_eq!(traced.len(), 2);
    let spans = exporter.get_finished_spans().unwrap();
    assert!(spans
        .iter()
        .any(|span| span.name == "background_task.process"
            && span.status == opentelemetry::trace::Status::error("fixture retry")));
    assert_eq!(
        spans
            .iter()
            .filter(|span| span.name == "background_task.process")
            .count(),
        2
    );
}

async fn assert_attempt_metrics(metrics: &WorkerMetrics) {
    let body = axum::body::to_bytes(metrics.render_response().into_body(), 1024 * 1024)
        .await
        .unwrap();
    let output = String::from_utf8(body.to_vec()).unwrap();
    for expected in [
        "worker_history_test_task_invocations_total{type=\"rebuild_fitness_freshness\"} 2",
        "worker_history_test_tasks_failed_total{type=\"rebuild_fitness_freshness\"} 1",
        "worker_history_test_tasks_completed_total{type=\"rebuild_fitness_freshness\"} 1",
    ] {
        assert!(output.contains(expected), "Missing metric: {expected}");
    }
}
