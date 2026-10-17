use super::*;
use crate::{
    auth::entities::users,
    test_support::{
        self,
        trace::{attribute, capture, stage},
    },
};
use opentelemetry::trace::{Span as _, TraceContextExt, Tracer, TracerProvider};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SimpleSpanProcessor};
use sea_orm::{DatabaseBackend, MockDatabase};
use tracing::Instrument;

#[tokio::test]
async fn repeated_queries_keep_full_counts_and_bounded_samples() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results((1..=100).map(|id| {
            vec![users::Model {
                id,
                ..test_support::customer()
            }]
        }))
        .into_connection();
    let trace = capture(async {
        async {
            for id in 1..=100 {
                assert_eq!(users::Model::find_by_id(&db, id).await.unwrap().id, id);
            }
        }
        .instrument(tracing::info_span!(
            "expensive_operation",
            bike.operation = "training.rebuild"
        ))
        .await;
    })
    .await;
    let operation = stage(&trace, "expensive_operation");
    assert_eq!(attribute(&operation, "bike.db.calls"), &100_i64.into());
    let spans = trace.spans();
    let queries: Vec<_> = spans.iter().filter(|span| span.name.starts_with("sea_orm.")).collect();
    // CPU throttling can make an in-memory query slow. Its separate sampling
    // budget still applies; elapsed time is not a mock-database contract.
    let slow = queries.iter().filter(|span| is_slow(span)).count();
    assert!(queries.len() - slow <= 1, "Repeated fast SQL needs at most one sample");
    assert!(slow <= 2, "Slow SQL has an independent two-sample budget");
    assert!(!queries.is_empty());
    let kept = i64::try_from(queries.len()).unwrap();
    assert_eq!(attribute(&operation, "bike.db.spans_kept"), &kept.into());
    assert_eq!(attribute(&operation, "bike.db.spans_dropped"), &(100 - kept).into());
    let request = spans.iter().find(|span| span.name == "request").unwrap();
    assert_eq!(attribute(request, "bike.db.calls"), &100_i64.into());
    assert_eq!(db.into_transaction_log().len(), 100);
}

#[test]
fn repeated_fast_queries_keep_exactly_one_sample_and_full_totals() {
    let (query, parent) = span_fixtures();
    let exporter = InMemorySpanExporter::default();
    let processor = DatabaseSpanProcessor::new(SimpleSpanProcessor::new(exporter.clone()));
    for index in 0..100 {
        processor.on_end(query_attempt(&query, index, Duration::from_millis(1)));
    }
    processor.on_end(parent);
    processor.force_flush().unwrap();
    let spans = exporter.get_finished_spans().unwrap();
    assert_eq!(spans.len(), 2);
    let parent = spans.iter().find(|span| span.name == "operation").unwrap();
    assert_eq!(attribute(parent, "bike.db.calls"), &100_i64.into());
    assert_eq!(attribute(parent, "bike.db.duration_ms"), &100_f64.into());
    assert_eq!(attribute(parent, "bike.db.spans_kept"), &1_i64.into());
    assert_eq!(attribute(parent, "bike.db.spans_dropped"), &99_i64.into());
}

#[test]
fn slow_sampling_starts_at_100_ms_and_preserves_its_separate_budget() {
    let (query, _) = span_fixtures();
    let mut work = DatabaseWork::default();
    let durations = [1, 99, 100, 100, 100];
    let expected = [true, false, true, true, false];
    for (index, (milliseconds, keep)) in durations.into_iter().zip(expected).enumerate() {
        let span = query_attempt(&query, index as u64, Duration::from_millis(milliseconds));
        assert_eq!(work.observe(&span), keep);
    }
    assert_eq!((work.calls, work.kept, work.dropped), (5, 3, 2));
    assert_eq!(work.duration_ms, 400.0);
}

#[test]
fn slow_and_failed_queries_have_separate_bounded_samples_and_exact_totals() {
    let (query, parent) = span_fixtures();
    let exporter = InMemorySpanExporter::default();
    let processor = DatabaseSpanProcessor::new(SimpleSpanProcessor::new(exporter.clone()));
    for index in 0..100_u64 {
        let mut span = query_attempt(&query, index, Duration::from_millis(200));
        if index >= 50 {
            span.status = Status::error("fixture database error");
        }
        processor.on_end(span);
    }
    processor.on_end(parent);
    processor.force_flush().unwrap();
    let spans = exporter.get_finished_spans().unwrap();
    assert_eq!(spans.len(), 5);
    assert_eq!(
        spans
            .iter()
            .filter(|span| matches!(span.status, Status::Error { .. }))
            .count(),
        2
    );
    let parent = spans.iter().find(|span| span.name == "operation").unwrap();
    assert_eq!(attribute(parent, "bike.db.calls"), &100_i64.into());
    assert_eq!(attribute(parent, "bike.db.duration_ms"), &20_000_f64.into());
    assert_eq!(attribute(parent, "bike.db.max_duration_ms"), &200_f64.into());
    assert_eq!(attribute(parent, "bike.db.errors"), &50_i64.into());
    assert_eq!(attribute(parent, "bike.db.spans_dropped"), &96_i64.into());
    assert!(processor.pending.lock().unwrap().is_empty());
}

#[test]
fn distinct_statement_samples_are_capped_independently_for_each_parent() {
    let (query, parent) = span_fixtures();
    let mut work = DatabaseWork::default();
    for index in 0..100 {
        let mut span = query.clone();
        span.attributes = vec![KeyValue::new("db.statement", format!("SELECT {index}"))];
        assert_eq!(work.observe(&span), index < 3);
    }
    assert_eq!((work.calls, work.kept, work.dropped), (100, 3, 97));
    let mut other = DatabaseWork::default();
    assert!(other.observe(&query));
    assert_ne!(query.parent_span_id, parent.parent_span_id);
}

#[tokio::test]
async fn real_database_failures_remain_visible_after_repeated_detail_is_dropped() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_errors((0..5).map(|_| sea_orm::DbErr::Custom("fixture failure".into())))
        .into_connection();
    let trace = capture(async {
        async {
            for _ in 0..5 {
                assert!(users::Model::find_by_id(&db, 7).await.is_err());
            }
        }
        .instrument(tracing::info_span!(
            "failed_operation",
            bike.operation = "training.rebuild"
        ))
        .await;
    })
    .await;
    let operation = stage(&trace, "failed_operation");
    assert_eq!(attribute(&operation, "bike.db.calls"), &5_i64.into());
    assert_eq!(attribute(&operation, "bike.db.errors"), &5_i64.into());
    assert_eq!(attribute(&operation, "bike.db.spans_kept"), &2_i64.into());
    assert_eq!(attribute(&operation, "bike.db.spans_dropped"), &3_i64.into());
}

#[test]
fn remote_parents_do_not_leave_completed_operation_summaries_in_memory() {
    let (query, mut parent) = span_fixtures();
    parent.parent_span_is_remote = true;
    parent.parent_span_id = opentelemetry::trace::SpanId::from_bytes([1; 8]);
    let processor = DatabaseSpanProcessor::new(SimpleSpanProcessor::new(InMemorySpanExporter::default()));
    processor.on_end(query);
    processor.on_end(parent);
    assert!(processor.pending.lock().unwrap().is_empty());
    processor.shutdown_with_timeout(Duration::from_secs(1)).unwrap();
}

#[test]
fn shutdown_clears_summaries_for_operations_that_have_not_closed() {
    let (query, _) = span_fixtures();
    let processor = DatabaseSpanProcessor::new(SimpleSpanProcessor::new(InMemorySpanExporter::default()));
    processor.on_end(query);
    assert_eq!(processor.pending.lock().unwrap().len(), 1);
    processor.shutdown_with_timeout(Duration::from_secs(1)).unwrap();
    assert!(processor.pending.lock().unwrap().is_empty());
}

fn span_fixtures() -> (SpanData, SpanData) {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let tracer = provider.tracer("query-fixture");
    let context = Context::new().with_span(tracer.start("operation"));
    let mut query = tracer.start_with_context("sea_orm.query_one", &context);
    query.set_attribute(KeyValue::new("db.statement", "SELECT id FROM users WHERE id = $1"));
    query.end();
    context.span().end();
    provider.force_flush().unwrap();
    let spans = exporter.get_finished_spans().unwrap();
    let mut query = spans[0].clone();
    query.end_time = query.start_time + Duration::from_millis(1);
    let mut parent = spans[1].clone();
    parent.end_time = query.end_time + Duration::from_secs(1);
    (query, parent)
}

fn query_attempt(query: &SpanData, index: u64, duration: Duration) -> SpanData {
    let mut span = query.clone();
    span.span_context = opentelemetry::trace::SpanContext::new(
        query.span_context.trace_id(),
        opentelemetry::trace::SpanId::from_bytes((index + 1).to_be_bytes()),
        query.span_context.trace_flags(),
        false,
        query.span_context.trace_state().clone(),
    );
    span.end_time = span.start_time + duration;
    span
}

fn is_slow(span: &SpanData) -> bool {
    span.end_time.duration_since(span.start_time).unwrap() >= Duration::from_millis(100)
}
