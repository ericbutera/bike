use crate::background_jobs::background_tasks;
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
        tokio::time::sleep(interval).await;
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
        let count = tasks.len();
        debug!(count, "Found pending task batch");

        for task_model in tasks {
            if let Err(worker_error) = self.process_task(task_model).await {
                error!(%worker_error, "Failed to process task");
            }
        }

        Ok(count)
    }

    async fn process_task(&self, task_model: background_tasks::Model) -> Result<(), WorkerError> {
        let task_id = task_model.id;
        let task_type = task_model.task_type.clone();
        let task_span = tracing::info_span!(
            "background_task.process",
            task.id = task_id,
            task.type = task_type.as_str(),
            task.status = tracing::field::Empty,
            error = tracing::field::Empty,
        );
        let task_span_for_records = task_span.clone();

        async move {
            info!(
                task_id,
                task_type = task_type.as_str(),
                "Starting background task"
            );

            if let Some(metrics) = &self.metrics {
                metrics.record_invocation(task_type.as_str());
                let lag_seconds =
                    (chrono::Utc::now() - task_model.created_at).num_milliseconds() as f64 / 1000.0;
                metrics.record_processing_lag(task_type.as_str(), lag_seconds);
            }

            let task_model = task_model.mark_processing(&self.db).await?;
            let started_at = std::time::Instant::now();
            let heartbeat = spawn_processing_heartbeat(
                self.db.clone(),
                task_model.id,
                task_model.task_type.clone(),
                Duration::from_secs(30),
            );

            let result = match self.processors.get(task_type.as_str()) {
                Some(processor) => {
                    processor
                        .process(task_model.id, task_model.payload.clone())
                        .await
                }
                None => Err(format!("No processor registered for task type: {}", task_type).into()),
            };
            heartbeat.abort();

            if let Some(metrics) = &self.metrics {
                metrics.record_duration(task_type.as_str(), started_at.elapsed().as_secs_f64());
            }

            match result {
                Ok(()) => {
                    task_span_for_records.record("task.status", "completed");
                    task_model.mark_completed(&self.db).await?;
                    info!(
                        task_id,
                        task_type = task_type.as_str(),
                        "Completed background task"
                    );
                    if let Some(metrics) = &self.metrics {
                        metrics.record_completed(task_type.as_str());
                    }
                }
                Err(process_error) => {
                    let error_message = process_error.to_string();
                    task_span_for_records.record("task.status", "failed");
                    task_span_for_records.record("error", error_message.as_str());
                    task_model
                        .mark_failed(&self.db, error_message.clone())
                        .await?;
                    warn!(
                        task_id,
                        task_type = task_type.as_str(),
                        error = %error_message,
                        "Failed background task"
                    );
                    if let Some(metrics) = &self.metrics {
                        metrics.record_failed(task_type.as_str());
                    }
                }
            }

            Ok(())
        }
        .instrument(task_span)
        .await
    }

    async fn run_startup_hooks(&self) {
        for hook in &self.startup_hooks {
            let hook_name = hook.name();
            let hook_span = tracing::info_span!(
                "background_task.startup_hook",
                hook.name = hook_name,
                hook.status = tracing::field::Empty,
                error = tracing::field::Empty,
            );

            async {
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
            }
            .instrument(hook_span.clone())
            .await;
        }
    }
}

fn spawn_processing_heartbeat(
    db: DatabaseConnection,
    task_id: i32,
    task_type: String,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;
            match background_tasks::Model::mark_processing_heartbeat(&db, task_id).await {
                Ok(Some(task))
                    if task.status == background_tasks::TaskStatus::Processing.as_str() =>
                {
                    debug!(task_id, task_type, "Recorded background task heartbeat");
                }
                Ok(_) => break,
                Err(error) => {
                    warn!(task_id, task_type, %error, "Failed to record background task heartbeat");
                }
            }
        }
    })
}
