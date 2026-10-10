use crate::background_jobs::{execution::WorkCounts, worker::catalog::ProcessorDefinition};
use prometheus::{
    Counter, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
};

pub(super) struct DiagnosticMetrics {
    timings: TimingMetrics,
    health: HealthMetrics,
    work: WorkMetrics,
    capacity: CapacityMetrics,
}
struct TimingMetrics {
    retries: IntCounterVec,
    attempts: HistogramVec,
    wait: HistogramVec,
    logical: HistogramVec,
    current: HistogramVec,
    available: HistogramVec,
}
impl TimingMetrics {
    fn new(registry: &Registry) -> Self {
        Self {
            retries: counter(
                registry,
                "task_retries_total",
                "Attempts returned for retry",
                &["type"],
            ),
            attempts: histogram(
                registry,
                "attempt_duration_seconds",
                "Attempt runtime by outcome",
                &["type", "outcome"],
            ),
            wait: histogram(
                registry,
                "eligible_wait_seconds",
                "Eligible queue wait by terminal attempt outcome",
                &["type", "outcome"],
            ),
            logical: histogram(
                registry,
                "logical_completion_seconds",
                "Task creation to successful completion including waits and retries",
                &["type"],
            ),
            current: histogram(
                registry,
                "receipt_to_task_seconds",
                "Original receipt to a claimed attempt",
                &["type", "entrypoint"],
            ),
            available: histogram(
                registry,
                "received_to_available_seconds",
                "Original receipt to revision-matched required outputs",
                &["entrypoint"],
            ),
        }
    }
}
struct HealthMetrics {
    registered: IntGaugeVec,
    states: IntGaugeVec,
    ages: IntGaugeVec,
    budgets: IntGaugeVec,
    anomaly_transitions: IntCounterVec,
    active_anomalies: IntGaugeVec,
    outputs: IntGaugeVec,
    unready: IntGaugeVec,
}
impl HealthMetrics {
    fn new(registry: &Registry) -> Self {
        Self {
            registered: gauge(
                registry,
                "processor_registered",
                "Processor present in this worker",
                &["type"],
            ),
            states: gauge(
                registry,
                "task_state_count",
                "Durable task states across the shared queue",
                &["type", "state"],
            ),
            ages: gauge(
                registry,
                "task_age_seconds",
                "Oldest eligible runtime heartbeat or meaningful progress age",
                &["type", "signal"],
            ),
            budgets: gauge(
                registry,
                "processor_budget_seconds",
                "Configured initial processor budgets",
                &["type", "signal"],
            ),
            anomaly_transitions: counter(
                registry,
                "task_anomalies_total",
                "New anomaly transitions",
                &["type", "reason"],
            ),
            active_anomalies: gauge(
                registry,
                "active_anomalies",
                "Active durable anomaly records",
                &["type", "reason"],
            ),
            outputs: gauge(
                registry,
                "pending_outputs",
                "Required output barriers still pending",
                &["kind"],
            ),
            unready: gauge(
                registry,
                "oldest_unready_seconds",
                "Oldest pending original receipt age",
                &["entrypoint"],
            ),
        }
    }
}
struct WorkMetrics {
    work_units: IntCounterVec,
    work_volume: IntCounterVec,
    work_duration: HistogramVec,
}
impl WorkMetrics {
    fn new(registry: &Registry) -> Self {
        Self {
            work_units: counter(
                registry,
                "work_units_total",
                "Actual inline work units, not multiplied by related runs",
                &["type", "kind", "mode", "reason", "outcome"],
            ),
            work_volume: counter(
                registry,
                "work_volume_total",
                "Scoped reads computations writes and publications",
                &["type", "kind", "mode", "unit"],
            ),
            work_duration: histogram(
                registry,
                "work_duration_seconds",
                "Inline work duration",
                &["type", "kind", "mode", "outcome"],
            ),
        }
    }
}
struct CapacityMetrics {
    busy: IntGauge,
    busy_seconds: Counter,
    idle_seconds: Counter,
}
impl CapacityMetrics {
    fn new(registry: &Registry) -> Self {
        let busy = IntGauge::new(
            "busy",
            "Whether this sequential worker is executing a processor",
        )
        .expect("busy gauge");
        let busy_seconds = Counter::new("busy_seconds_total", "Executed worker wall seconds")
            .expect("busy counter");
        let idle_seconds =
            Counter::new("idle_seconds_total", "Worker polling/backoff wall seconds")
                .expect("idle counter");
        registry
            .register(Box::new(busy.clone()))
            .expect("busy registration");
        registry
            .register(Box::new(busy_seconds.clone()))
            .expect("busy counter registration");
        registry
            .register(Box::new(idle_seconds.clone()))
            .expect("idle counter registration");
        Self {
            busy,
            busy_seconds,
            idle_seconds,
        }
    }
}
impl DiagnosticMetrics {
    pub(super) fn new(registry: &Registry) -> Self {
        Self {
            timings: TimingMetrics::new(registry),
            health: HealthMetrics::new(registry),
            work: WorkMetrics::new(registry),
            capacity: CapacityMetrics::new(registry),
        }
    }
    pub(super) fn warmup(&self, task_types: &[&str]) {
        for task_type in task_types {
            self.health
                .registered
                .with_label_values(&[task_type])
                .set(1);
            self.timings
                .retries
                .with_label_values(&[task_type])
                .inc_by(0);
            self.timings.logical.with_label_values(&[task_type]);
            let definition = ProcessorDefinition::for_type(task_type);
            for (signal, budget) in [
                ("eligible", definition.queue_budget_seconds),
                ("runtime", definition.runtime_budget_seconds),
                ("heartbeat", 120),
                ("progress", definition.progress_budget_seconds),
            ] {
                self.health
                    .ages
                    .with_label_values(&[task_type, signal])
                    .set(0);
                self.health
                    .budgets
                    .with_label_values(&[task_type, signal])
                    .set(budget);
            }
            for state in [
                "queued",
                "scheduled",
                "retrying",
                "running",
                "failed",
                "completed",
            ] {
                self.health
                    .states
                    .with_label_values(&[task_type, state])
                    .set(0);
            }
        }
    }
}

impl super::WorkerMetrics {
    pub fn record_eligible_wait(&self, task_type: &str, outcome: &str, seconds: f64) {
        if seconds >= 0.0 {
            self.diagnostics
                .timings
                .wait
                .with_label_values(&[task_type, outcome])
                .observe(seconds);
        }
    }
    pub fn record_retry(&self, task_type: &str) {
        self.diagnostics
            .timings
            .retries
            .with_label_values(&[task_type])
            .inc();
    }
    pub fn record_attempt_duration(&self, task_type: &str, outcome: &str, seconds: f64) {
        self.diagnostics
            .timings
            .attempts
            .with_label_values(&[task_type, outcome])
            .observe(seconds);
    }
    pub fn record_logical_duration(&self, task_type: &str, seconds: f64) {
        self.diagnostics
            .timings
            .logical
            .with_label_values(&[task_type])
            .observe(seconds);
    }
    pub fn record_receipt_to_current(&self, task_type: &str, entrypoint: &str, seconds: f64) {
        if seconds >= 0.0 {
            self.diagnostics
                .timings
                .current
                .with_label_values(&[task_type, entrypoint])
                .observe(seconds);
        }
    }
    pub fn record_available(&self, entrypoint: &str, seconds: f64) {
        if seconds >= 0.0 {
            self.diagnostics
                .timings
                .available
                .with_label_values(&[entrypoint])
                .observe(seconds);
        }
    }
    pub fn set_busy(&self, busy: bool) {
        self.diagnostics.capacity.busy.set(i64::from(busy));
    }
    pub fn record_busy(&self, seconds: f64) {
        self.diagnostics.capacity.busy_seconds.inc_by(seconds);
    }
    pub fn record_idle(&self, seconds: f64) {
        self.diagnostics.capacity.idle_seconds.inc_by(seconds);
    }
    pub fn set_state(&self, task_type: &str, state: &str, count: i64) {
        self.diagnostics
            .health
            .states
            .with_label_values(&[task_type, state])
            .set(count);
    }
    pub fn set_age(&self, task_type: &str, signal: &str, seconds: i64) {
        self.diagnostics
            .health
            .ages
            .with_label_values(&[task_type, signal])
            .set(seconds);
    }
    pub fn record_anomaly(&self, task_type: &str, reason: &str) {
        self.diagnostics
            .health
            .anomaly_transitions
            .with_label_values(&[task_type, reason])
            .inc();
    }
    pub fn set_anomalies(&self, task_type: &str, reason: &str, count: i64) {
        self.diagnostics
            .health
            .active_anomalies
            .with_label_values(&[task_type, reason])
            .set(count);
    }
    pub fn set_pending_outputs(&self, kind: &str, count: i64) {
        self.diagnostics
            .health
            .outputs
            .with_label_values(&[kind])
            .set(count);
    }
    pub fn set_unready_age(&self, entrypoint: &str, seconds: i64) {
        self.diagnostics
            .health
            .unready
            .with_label_values(&[entrypoint])
            .set(seconds);
    }
    pub fn record_work(
        &self,
        task_type: &str,
        kind: &str,
        mode: &str,
        reason: &str,
        counts: Option<&WorkCounts>,
        seconds: f64,
    ) {
        let outcome = counts.map(|counts| counts.outcome).unwrap_or("failed");
        self.diagnostics
            .work
            .work_units
            .with_label_values(&[task_type, kind, mode, reason, outcome])
            .inc();
        self.diagnostics
            .work
            .work_duration
            .with_label_values(&[task_type, kind, mode, outcome])
            .observe(seconds);
        if let Some(counts) = counts {
            for (unit, count) in [
                ("inputs_read", counts.inputs_read),
                ("units_computed", counts.units_computed),
                ("rows_written", counts.rows_written),
                ("outputs_published", counts.outputs_published),
            ] {
                self.diagnostics
                    .work
                    .work_volume
                    .with_label_values(&[task_type, kind, mode, unit])
                    .inc_by(count);
            }
        }
    }
}

fn counter(registry: &Registry, name: &str, help: &str, labels: &[&str]) -> IntCounterVec {
    let metric = IntCounterVec::new(Opts::new(name, help), labels).expect("worker counter");
    registry
        .register(Box::new(metric.clone()))
        .expect("counter registration");
    metric
}
fn histogram(registry: &Registry, name: &str, help: &str, labels: &[&str]) -> HistogramVec {
    let metric = HistogramVec::new(
        HistogramOpts::new(name, help).buckets(super::DURATION_BUCKETS.to_vec()),
        labels,
    )
    .expect("worker histogram");
    registry
        .register(Box::new(metric.clone()))
        .expect("histogram registration");
    metric
}
fn gauge(registry: &Registry, name: &str, help: &str, labels: &[&str]) -> IntGaugeVec {
    let metric = IntGaugeVec::new(Opts::new(name, help), labels).expect("worker gauge");
    registry
        .register(Box::new(metric.clone()))
        .expect("gauge registration");
    metric
}
