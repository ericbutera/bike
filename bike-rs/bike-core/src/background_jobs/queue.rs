use crate::background_jobs::error::TaskError;
use crate::background_jobs::storage::{TaskRecord, TaskStorage};
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, Instrument};

/// Enqueues background tasks with trace and pipeline context.
pub struct TaskQueue<S: TaskStorage> {
    storage: Arc<S>,
}

impl<S: TaskStorage> TaskQueue<S> {
    pub fn new(storage: S) -> Self {
        Self {
            storage: Arc::new(storage),
        }
    }

    /// Enqueue a task to be processed in the background
    pub async fn enqueue<T: serde::Serialize>(
        &self,
        task_type: String,
        task: T,
    ) -> Result<TaskRecord, TaskError> {
        self.enqueue_with_options(task_type, task, None, 3).await
    }

    /// Enqueue a task with custom options
    pub async fn enqueue_with_options<T: serde::Serialize>(
        &self,
        task_type: String,
        task: T,
        scheduled_for: Option<chrono::DateTime<Utc>>,
        max_attempts: i32,
    ) -> Result<TaskRecord, TaskError> {
        let span = tracing::info_span!(
            "background_task.enqueue",
            task.type = task_type.as_str(),
            task.scheduled = scheduled_for.is_some(),
            task.max_attempts = max_attempts,
            task.id = tracing::field::Empty,
        );
        let span_for_records = span.clone();

        async move {
            let mut payload = serde_json::to_value(&task)?;
            super::pipeline::PipelineContext::attach(&mut payload)?;

            let task_record = self
                .storage
                .enqueue(task_type.clone(), payload, scheduled_for, max_attempts)
                .await?;

            span_for_records.record("task.id", task_record.id.as_str());
            debug!(
                "Task enqueued successfully: id={}, type={}",
                task_record.id, task_type
            );

            Ok(task_record)
        }
        .instrument(span.clone())
        .await
    }
}

impl<S: TaskStorage> Clone for TaskQueue<S> {
    fn clone(&self) -> Self {
        Self {
            storage: Arc::clone(&self.storage),
        }
    }
}
