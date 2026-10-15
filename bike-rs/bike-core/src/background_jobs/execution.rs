//! Shared inline-work evidence, kept separate from authoritative domain outputs.
use super::{entities::work_units, worker::WorkerMetrics};
use chrono::Utc;
use sea_orm::ConnectionTrait;
use serde::Serialize;
use std::{fmt::Display, future::Future, sync::Arc};
use tracing::Instrument;

tokio::task_local! { static CURRENT: ExecutionContext; }

#[derive(Clone)]
pub struct ExecutionContext {
    pub task_id: i32,
    pub attempt: i32,
    pub task_type: String,
    pub metrics: Option<Arc<WorkerMetrics>>,
}

impl ExecutionContext {
    pub async fn scope<F: Future>(self, future: F) -> F::Output {
        CURRENT.scope(self, future).await
    }
    pub fn current() -> Option<Self> {
        CURRENT.try_with(Clone::clone).ok()
    }
    pub async fn record_progress(&self, db: &impl ConnectionTrait) {
        if let Err(error) =
            super::entities::task_attempts::Model::progress(db, self.task_id, self.attempt).await
        {
            tracing::error!(%error, "Failed to record meaningful worker progress");
        }
    }
}

#[derive(Clone, Debug)]
pub struct WorkScope {
    pub kind: &'static str,
    pub work_key: String,
    pub revision: String,
    pub mode: &'static str,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct WorkCounts {
    pub inputs_read: u64,
    pub units_computed: u64,
    pub rows_written: u64,
    pub outputs_published: u64,
    pub outcome: &'static str,
}

/// A diagnostic write failure does not undo successful business work. It is
/// logged with the task context and stays visibly missing in durable history.
pub async fn measured<T, E, F>(
    db: &impl ConnectionTrait,
    scope: WorkScope,
    future: F,
) -> Result<T, E>
where
    E: Display,
    F: Future<Output = Result<(T, WorkCounts), E>>,
{
    let context = ExecutionContext::current();
    let span = tracing::info_span!(
        "processor.inline_work",
        work_kind = scope.kind,
        work_key = scope.work_key,
        revision = scope.revision,
        mode = scope.mode,
        reason = scope.reason,
        trace_id = tracing::field::Empty,
        span_id = tracing::field::Empty,
        "otel.status_code" = tracing::field::Empty,
        error = tracing::field::Empty
    );
    async move {
        crate::observability::record_current_trace_context();
        let started_at = Utc::now();
        let clock = std::time::Instant::now();
        let result = future.await;
        let finished_at = Utc::now();
        let duration = clock.elapsed().as_secs_f64();
        let counts = result.as_ref().ok().map(|(_, counts)| counts);
        let error = result.as_ref().err().map(ToString::to_string);
        if let Some(error) = &error {
            tracing::Span::current().record("otel.status_code", "ERROR");
            tracing::Span::current().record("error", error.as_str());
        }
        if let Some(context) = context {
            if let Err(error) = work_units::Model::record(
                db,
                &context,
                &scope,
                started_at,
                finished_at,
                counts,
                error,
            )
            .await
            {
                tracing::error!(%error, "Failed to persist inline work evidence");
            }
            if let Some(metrics) = &context.metrics {
                metrics.record_work(
                    &context.task_type,
                    scope.kind,
                    scope.mode,
                    scope.reason,
                    counts,
                    duration,
                );
            }
            context.record_progress(db).await;
        }
        result.map(|(value, _)| value)
    }
    .instrument(span)
    .await
}
