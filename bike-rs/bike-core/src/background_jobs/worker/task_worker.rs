use crate::background_jobs::background_tasks;
use crate::background_jobs::pipeline::PipelineContext;
use crate::background_jobs::worker::metrics::WorkerMetrics;
use crate::background_jobs::worker::processor::TaskProcessor;
use crate::background_jobs::worker::startup::WorkerStartupHook;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn, Instrument};

pub type WorkerError = Box<dyn std::error::Error + Send + Sync>;

pub struct TaskWorker {
    db: DatabaseConnection,
    batch_size: u64,
    poll_interval: Duration,
    processors: HashMap<String, Arc<dyn TaskProcessor>>,
    startup_hooks: Vec<Arc<dyn WorkerStartupHook>>,
    metrics: Option<Arc<WorkerMetrics>>,
}

impl TaskWorker {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            batch_size: 10,
            poll_interval: Duration::from_secs(1),
            processors: HashMap::new(),
            startup_hooks: Vec::new(),
            metrics: None,
        }
    }

    pub fn with_batch_size(mut self, batch_size: u64) -> Self {
        self.batch_size = batch_size;
        self
    }

    pub fn with_poll_interval(mut self, poll_interval: Duration) -> Self {
        self.poll_interval = poll_interval;
        self
    }

    pub fn with_metrics(mut self, metrics: Arc<WorkerMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    pub fn register_processor(mut self, processor: Arc<dyn TaskProcessor>) -> Self {
        self.processors
            .insert(processor.task_type().to_string(), processor);
        self
    }

    pub fn register_startup_hook(mut self, hook: Arc<dyn WorkerStartupHook>) -> Self {
        self.startup_hooks.push(hook);
        self
    }

    pub fn registered_task_types(&self) -> Vec<String> {
        self.processors.keys().cloned().collect()
    }

    pub async fn run(self) {
        debug!(
            "Task worker started (batch_size={}, poll_interval={:?}, processors={}, startup_hooks={})",
            self.batch_size,
            self.poll_interval,
            self.processors.len(),
            self.startup_hooks.len()
        );

        self.run_startup_hooks().await;

        self.poll().await;
    }

    async fn poll(&self) {
        let mut current_interval = self.poll_interval;
        let max_backoff = Duration::from_secs(60);

        loop {
            match self.process_batch().await {
                Ok(processed) if processed > 0 => {
                    current_interval = self.poll_interval;
                }
                Ok(_) => {
                    current_interval = self.backoff(current_interval, max_backoff).await;
                }
                Err(worker_error) => {
                    error!(%worker_error, "Error processing task batch");
                    current_interval = self.backoff(current_interval, max_backoff).await;
                }
            }
        }
    }

    async fn backoff(&self, interval: Duration, maximum: Duration) -> Duration {
        let started = std::time::Instant::now();
        tokio::time::sleep(interval).await;
        if let Some(metrics) = &self.metrics {
            metrics.record_idle(started.elapsed().as_secs_f64());
        }
        Duration::from_secs(
            interval
                .as_secs()
                .saturating_mul(2)
                .min(maximum.as_secs())
                .max(1),
        )
    }

    async fn process_batch(&self) -> Result<usize, WorkerError> {
        let tasks = background_tasks::Model::find_pending(&self.db, self.batch_size).await?;
        let mut count = 0;
        debug!(count = tasks.len(), "Found pending task batch");

        for task_model in tasks {
            match self.process_task(task_model).await {
                Ok(true) => count += 1,
                Ok(false) => (),
                Err(worker_error) => error!(%worker_error, "Failed to process task"),
            }
        }

        Ok(count)
    }

    async fn process_task(&self, pending: background_tasks::Model) -> Result<bool, WorkerError> {
        let Some(task) = pending.claim(&self.db).await? else {
            return Ok(false);
        };
        let context = PipelineContext::from_payload(&task.payload);
        let origin = context.as_ref().ok().and_then(Option::as_ref);
        let task_span = tracing::info_span!(
            "background_task.process",
            task.id = task.id,
            task.type = task.task_type.as_str(),
            task.attempt = task.attempts,
            pipeline_run_id = origin.map(|context| context.run_id.as_str()),
            pipeline_started_at = origin.map(|context| context.pipeline_started_at.to_rfc3339()),
            request_id = origin.and_then(|context| context.request_id.as_deref()),
            trace_id = tracing::field::Empty,
            span_id = tracing::field::Empty,
            task.status = tracing::field::Empty,
            "otel.status_code" = tracing::field::Empty,
            "otel.status_description" = tracing::field::Empty,
            error = tracing::field::Empty,
        );
        if let Some(carrier) = origin.and_then(|context| context.trace_context.as_ref()) {
            crate::observability::set_span_parent_from_carrier(&task_span, Some(carrier));
        }
        let execution_context = origin.cloned().map(|context| context.for_task(task.id));
        PipelineContext::scope(
            execution_context,
            crate::background_jobs::execution::ExecutionContext {
                task_id: task.id,
                attempt: task.attempts,
                task_type: task.task_type.clone(),
                metrics: self.metrics.clone(),
            }
            .scope(crate::background_jobs::batches::scope(
                task.payload["_batch_id"]
                    .as_i64()
                    .and_then(|id| i32::try_from(id).ok()),
                self.execute_claimed(&task, context.err()),
            )),
        )
        .instrument(task_span)
        .await?;
        Ok(true)
    }

    async fn execute_claimed(
        &self,
        task: &background_tasks::Model,
        context_error: Option<serde_json::Error>,
    ) -> Result<(), WorkerError> {
        crate::observability::record_current_trace_context();
        task.record_execution_trace(&self.db).await?;
        info!("Starting background task");
        if let Some(metrics) = &self.metrics {
            metrics.set_busy(true);
            metrics.record_invocation(&task.task_type);
            if let Some(context) = PipelineContext::current() {
                metrics.record_receipt_to_current(
                    &task.task_type,
                    &context.entrypoint,
                    (chrono::Utc::now() - context.pipeline_started_at).num_milliseconds() as f64
                        / 1000.0,
                );
            }
        }
        let started_at = std::time::Instant::now();
        let heartbeat =
            spawn_processing_heartbeat(self.db.clone(), task.clone(), Duration::from_secs(30));
        let result = match context_error {
            Some(error) => Err(error.into()),
            None => self.invoke_with_capacity(task).await,
        };
        heartbeat.abort();
        if let Some(metrics) = &self.metrics {
            metrics.set_busy(false);
            metrics.record_duration(&task.task_type, started_at.elapsed().as_secs_f64());
        }
        self.record_result(task, result, started_at.elapsed().as_secs_f64())
            .await
    }

    async fn invoke_processor(&self, task: &background_tasks::Model) -> Result<(), WorkerError> {
        match self.processors.get(&task.task_type) {
            Some(processor) => processor.process(task.id, task.payload.clone()).await,
            None => {
                Err(format!("No processor registered for task type: {}", task.task_type).into())
            }
        }
    }

    async fn invoke_with_capacity(
        &self,
        task: &background_tasks::Model,
    ) -> Result<(), WorkerError> {
        let future = self.invoke_processor(task);
        tokio::pin!(future);
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        let mut recorded = std::time::Instant::now();
        loop {
            let result =
                tokio::select! { result=&mut future => Some(result), _=ticker.tick()=>None };
            if let Some(metrics) = &self.metrics {
                metrics.record_busy(recorded.elapsed().as_secs_f64());
            }
            recorded = std::time::Instant::now();
            if let Some(result) = result {
                return result;
            }
        }
    }

    async fn record_result(
        &self,
        task: &background_tasks::Model,
        result: Result<(), WorkerError>,
        duration: f64,
    ) -> Result<(), WorkerError> {
        let span = tracing::Span::current();
        let result = result.map_err(|error| error.to_string());
        if let Err(error) = &result {
            span.record("error", error.as_str());
            span.record("otel.status_code", "ERROR");
            span.record("otel.status_description", error.as_str());
        }
        let Some(updated) = task.finish_execution(&self.db, result).await? else {
            record_superseded_execution();
            return Ok(());
        };
        self.record_task_outcome(&task.task_type, &updated);
        self.record_diagnostic_result(&updated, duration).await;
        Ok(())
    }

    async fn record_diagnostic_result(&self, task: &background_tasks::Model, duration: f64) {
        let Some(metrics) = &self.metrics else {
            return;
        };
        self.record_attempt_wait(task, metrics).await;
        let outcome = if task.status == "pending" {
            "retrying"
        } else {
            &task.status
        };
        metrics.record_attempt_duration(&task.task_type, outcome, duration);
        if task.status == "pending" {
            metrics.record_retry(&task.task_type);
        }
        if task.status == "completed" {
            metrics.record_logical_duration(
                &task.task_type,
                (chrono::Utc::now() - task.created_at).num_milliseconds() as f64 / 1000.0,
            );
        }
        if let Err(error) =
            crate::background_jobs::diagnostics::completed(&self.db, metrics, task).await
        {
            error!(%error,"Failed to evaluate completed attempt diagnostics");
        }
    }

    async fn record_attempt_wait(&self, task: &background_tasks::Model, metrics: &WorkerMetrics) {
        let history = match crate::background_jobs::entities::task_attempts::Model::for_task(
            &self.db, task.id,
        )
        .await
        {
            Ok(history) => history,
            Err(error) => {
                error!(%error,"Failed to read attempt eligibility");
                return;
            }
        };
        let Some(attempt) = history.last() else {
            return;
        };
        let Some(eligible) = attempt.eligible_at else {
            return;
        };
        let seconds = (attempt.started_at - eligible).num_milliseconds() as f64 / 1000.0;
        metrics.record_eligible_wait(&task.task_type, &attempt.outcome, seconds);
        if seconds >= 0.0 {
            metrics.record_processing_lag(&task.task_type, seconds);
        }
    }

    fn record_task_outcome(&self, task_type: &str, updated: &background_tasks::Model) {
        tracing::Span::current().record("task.status", updated.status.as_str());
        if updated.status == "completed" {
            info!("Completed background task");
        } else {
            warn!(error = updated.error, "Background task failed");
        }
        self.record_outcome_metric(task_type, &updated.status);
    }

    fn record_outcome_metric(&self, task_type: &str, status: &str) {
        let Some(metrics) = &self.metrics else {
            return;
        };
        if status == "completed" {
            metrics.record_completed(task_type);
        } else {
            metrics.record_failed(task_type);
        }
    }

    async fn run_startup_hooks(&self) {
        for hook in &self.startup_hooks {
            let hook_name = hook.name();
            let context = PipelineContext::received("worker_startup", None);
            let hook_span = tracing::info_span!(
                "background_task.startup_hook",
                hook.name = hook_name,
                pipeline_run_id = context.run_id.as_str(),
                pipeline_started_at = %context.pipeline_started_at,
                trace_id = tracing::field::Empty,
                span_id = tracing::field::Empty,
                hook.status = tracing::field::Empty,
                error = tracing::field::Empty,
            );

            PipelineContext::scope(Some(context), async {
                crate::observability::record_current_trace_context();
                info!(hook = hook_name, "Running worker startup hook");

                match hook.run(&self.db).await {
                    Ok(()) => {
                        hook_span.record("hook.status", "completed");
                        info!(hook = hook_name, "Completed worker startup hook");
                    }
                    Err(worker_error) => {
                        hook_span.record("hook.status", "failed");
                        hook_span.record("error", worker_error.to_string().as_str());
                        error!(hook = hook_name, %worker_error, "Worker startup hook failed")
                    }
                }
            })
            .instrument(hook_span.clone())
            .await;
        }
    }
}

fn spawn_processing_heartbeat(
    db: DatabaseConnection,
    task: background_tasks::Model,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;
            match task.heartbeat(&db).await {
                Ok(true) => debug!(
                    task_id = task.id,
                    attempt = task.attempts,
                    "Recorded background task heartbeat"
                ),
                Ok(false) => break,
                Err(error) => {
                    warn!(task_id = task.id, attempt = task.attempts, %error, "Failed to record background task heartbeat");
                }
            }
        }
    })
}

fn record_superseded_execution() {
    tracing::Span::current().record("task.status", "superseded");
    info!("Discarded result from a superseded or canceled execution");
}

#[cfg(test)]
#[path = "task_worker_tests.rs"]
mod tests;
