//! Bound exported SQL detail while retaining complete operation-level totals.
use opentelemetry::{trace::Status, Context, KeyValue};
use opentelemetry_sdk::{
    error::OTelSdkResult,
    trace::{Span, SpanData, SpanProcessor},
    Resource,
};
use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::Mutex,
    time::Duration,
};

type SpanKey = (opentelemetry::trace::TraceId, opentelemetry::trace::SpanId);

#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
struct DatabaseWork {
    calls: i64,
    duration_ms: f64,
    max_duration_ms: f64,
    errors: i64,
    kept: i64,
    dropped: i64,
    statements: HashSet<u64>,
    slow_samples: usize,
    error_samples: usize,
}

impl DatabaseWork {
    fn observe(&mut self, span: &SpanData) -> bool {
        let duration = span.end_time.duration_since(span.start_time).unwrap_or_default();
        let duration_ms = duration.as_secs_f64() * 1000.0;
        let failed = matches!(span.status, Status::Error { .. });
        self.calls += 1;
        self.duration_ms += duration_ms;
        self.max_duration_ms = self.max_duration_ms.max(duration_ms);
        self.errors += i64::from(failed);
        let keep = if failed {
            take_sample(&mut self.error_samples)
        } else if duration >= Duration::from_millis(100) {
            take_sample(&mut self.slow_samples)
        } else {
            self.sample_statement(span)
        };
        if keep {
            self.kept += 1;
        } else {
            self.dropped += 1;
        }
        keep
    }

    fn sample_statement(&mut self, span: &SpanData) -> bool {
        if self.statements.len() == 3 {
            return false;
        }
        let Some(statement) = span
            .attributes
            .iter()
            .find(|field| field.key.as_str() == "db.statement")
        else {
            return false;
        };
        let mut fingerprint = DefaultHasher::new();
        span.name.hash(&mut fingerprint);
        statement.value.as_str().hash(&mut fingerprint);
        self.statements.insert(fingerprint.finish())
    }

    fn merge(&mut self, child: &Self) {
        self.calls += child.calls;
        self.duration_ms += child.duration_ms;
        self.max_duration_ms = self.max_duration_ms.max(child.max_duration_ms);
        self.errors += child.errors;
        self.kept += child.kept;
        self.dropped += child.dropped;
    }

    fn annotate(&self, span: &mut SpanData) {
        span.attributes.extend([
            KeyValue::new("bike.db.calls", self.calls),
            KeyValue::new("bike.db.duration_ms", self.duration_ms),
            KeyValue::new("bike.db.max_duration_ms", self.max_duration_ms),
            KeyValue::new("bike.db.errors", self.errors),
            KeyValue::new("bike.db.spans_kept", self.kept),
            KeyValue::new("bike.db.spans_dropped", self.dropped),
        ]);
    }
}

fn take_sample(samples: &mut usize) -> bool {
    if *samples == 2 {
        return false;
    }
    *samples += 1;
    true
}

#[derive(Debug)]
pub(crate) struct DatabaseSpanProcessor<P> {
    next: P,
    pending: Mutex<HashMap<SpanKey, DatabaseWork>>,
}

impl<P: SpanProcessor> DatabaseSpanProcessor<P> {
    pub(crate) fn new(next: P) -> Self {
        Self {
            next,
            pending: Mutex::default(),
        }
    }

    fn sample(&self, span: &SpanData) -> bool {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry((span.span_context.trace_id(), span.parent_span_id))
            .or_default()
            .observe(span)
    }

    fn summarize(&self, span: &mut SpanData) {
        let key = (span.span_context.trace_id(), span.span_context.span_id());
        let mut pending = self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(work) = pending.remove(&key) else {
            return;
        };
        work.annotate(span);
        // Remote parents belong to another execution, and may already be closed.
        if span.parent_span_is_remote || span.parent_span_id == opentelemetry::trace::SpanId::INVALID {
            return;
        }
        pending.entry((key.0, span.parent_span_id)).or_default().merge(&work);
    }
}

impl<P: SpanProcessor> SpanProcessor for DatabaseSpanProcessor<P> {
    fn on_start(&self, span: &mut Span, context: &Context) {
        self.next.on_start(span, context);
    }

    fn on_end(&self, mut span: SpanData) {
        if span.name.starts_with("sea_orm.") {
            if !self.sample(&span) {
                return;
            }
        } else {
            self.summarize(&mut span);
        }
        self.next.on_end(span);
    }

    fn force_flush(&self) -> OTelSdkResult {
        self.next.force_flush()
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        self.next.shutdown_with_timeout(timeout)
    }

    fn set_resource(&mut self, resource: &Resource) {
        self.next.set_resource(resource);
    }
}
