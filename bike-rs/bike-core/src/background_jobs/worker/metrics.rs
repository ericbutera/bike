use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::response::Response;
use axum::{routing::get, Router};
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry, TextEncoder,
};
use std::sync::Arc;

pub struct WorkerMetrics {
    registry: Registry,
    tasks_completed: IntCounterVec,
    tasks_failed: IntCounterVec,
    task_invocations: IntCounterVec,
    task_processing_lag: HistogramVec,
    task_duration_seconds: HistogramVec,
}

impl WorkerMetrics {
    pub fn new(namespace: &str) -> Self {
        let registry = Registry::new_custom(Some(namespace.to_string()), None)
            .expect("failed to create worker metrics registry");

        let tasks_completed = IntCounterVec::new(
            Opts::new(
                "tasks_completed_total",
                "Number of successfully completed background tasks",
            ),
            &["type"],
        )
        .expect("failed to create tasks_completed metric");
        registry
            .register(Box::new(tasks_completed.clone()))
            .expect("failed to register tasks_completed metric");

        let tasks_failed = IntCounterVec::new(
            Opts::new("tasks_failed_total", "Number of failed background tasks"),
            &["type"],
        )
        .expect("failed to create tasks_failed metric");
        registry
            .register(Box::new(tasks_failed.clone()))
            .expect("failed to register tasks_failed metric");

        let task_invocations = IntCounterVec::new(
            Opts::new(
                "task_invocations_total",
                "Number of task processing attempts",
            ),
            &["type"],
        )
        .expect("failed to create task_invocations metric");
        registry
            .register(Box::new(task_invocations.clone()))
            .expect("failed to register task_invocations metric");

        let task_processing_lag = HistogramVec::new(
            HistogramOpts::new(
                "task_processing_lag_seconds",
                "Time from task creation to processing start in seconds",
            )
            .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0]),
            &["type"],
        )
        .expect("failed to create task_processing_lag metric");
        registry
            .register(Box::new(task_processing_lag.clone()))
            .expect("failed to register task_processing_lag metric");

        let task_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "task_duration_seconds",
                "Task execution duration in seconds",
            )
            .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]),
            &["type"],
        )
        .expect("failed to create task_duration_seconds metric");
        registry
            .register(Box::new(task_duration_seconds.clone()))
            .expect("failed to register task_duration_seconds metric");

        Self {
            registry,
            tasks_completed,
            tasks_failed,
            task_invocations,
            task_processing_lag,
            task_duration_seconds,
        }
    }

    pub fn warmup_task_types(&self, task_types: &[&str]) {
        for task_type in task_types {
            self.tasks_completed
                .with_label_values(&[*task_type])
                .inc_by(0);
            self.tasks_failed.with_label_values(&[*task_type]).inc_by(0);
            self.task_invocations
                .with_label_values(&[*task_type])
                .inc_by(0);
            self.task_processing_lag
                .with_label_values(&[*task_type])
                .observe(0.0);
            self.task_duration_seconds
                .with_label_values(&[*task_type])
                .observe(0.0);
        }
    }

    pub fn record_invocation(&self, task_type: &str) {
        self.task_invocations.with_label_values(&[task_type]).inc();
    }

    pub fn record_processing_lag(&self, task_type: &str, lag_seconds: f64) {
        self.task_processing_lag
            .with_label_values(&[task_type])
            .observe(lag_seconds);
    }

    pub fn record_duration(&self, task_type: &str, duration_seconds: f64) {
        self.task_duration_seconds
            .with_label_values(&[task_type])
            .observe(duration_seconds);
    }

    pub fn record_completed(&self, task_type: &str) {
        self.tasks_completed.with_label_values(&[task_type]).inc();
    }

    pub fn record_failed(&self, task_type: &str) {
        self.tasks_failed.with_label_values(&[task_type]).inc();
    }

    pub fn render_response(&self) -> Response {
        let encoder = TextEncoder::new();
        let mut metric_families = self.registry.gather();
        metric_families.extend(prometheus::gather());
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .unwrap_or_default();
        let body = String::from_utf8(buffer).unwrap_or_default();
        Response::builder()
            .header(CONTENT_TYPE, encoder.format_type())
            .body(Body::from(body))
            .expect("failed to build metrics response")
    }
}

pub fn spawn_metrics_server(port: u16, metrics: Arc<WorkerMetrics>) {
    tokio::spawn(async move {
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        let app = Router::new().route(
            "/metrics",
            get({
                let metrics = metrics.clone();
                move || {
                    let metrics = metrics.clone();
                    async move { metrics.render_response() }
                }
            }),
        );

        tracing::debug!(%addr, "worker metrics server listening");

        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(error) => {
                tracing::error!(%error, %addr, "failed to bind worker metrics server");
                return;
            }
        };

        if let Err(error) = axum::serve(listener, app).await {
            tracing::error!(%error, "worker metrics server exited");
        }
    });
}
