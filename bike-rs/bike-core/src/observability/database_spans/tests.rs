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
async fn repeated_queries_keep_full_counts_and_one_statement_sample() {
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
    assert_eq!(attribute(&operation, "bike.db.spans_kept"), &1_i64.into());
    assert_eq!(
        attribute(&operation, "bike.db.spans_dropped"),
        &99_i64.into()
    );
    let spans = trace.spans();
    assert_eq!(
        spans
            .iter()
            .filter(|span| span.name.starts_with("sea_orm."))
            .count(),
        1
    );
    let request = spans.iter().find(|span| span.name == "request").unwrap();
    assert_eq!(attribute(request, "bike.db.calls"), &100_i64.into());
    assert_eq!(db.into_transaction_log().len(), 100);
}

#[test]
fn slow_and_failed_queries_have_separate_bounded_samples_and_exact_totals() {
    let (query, parent) = span_fixtures();
    let exporter = InMemorySpanExporter::default();
    let processor = DatabaseSpanProcessor::new(SimpleSpanProcessor::new(exporter.clone()));
    for index in 0..100_u64 {
        let mut span = query.clone();
        span.span_context = opentelemetry::trace::SpanContext::new(
            query.span_context.trace_id(),
            opentelemetry::trace::SpanId::from_bytes((index + 1).to_be_bytes()),
            query.span_context.trace_flags(),
            false,
            query.span_context.trace_state().clone(),
        );
        span.end_time = span.start_time + Duration::from_millis(200);
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
    assert_eq!(
        attribute(parent, "bike.db.max_duration_ms"),
        &200_f64.into()
    );
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
    assert_eq!(
        attribute(&operation, "bike.db.spans_dropped"),
        &3_i64.into()
    );
}

#[test]
fn remote_parents_do_not_leave_completed_operation_summaries_in_memory() {
    let (query, mut parent) = span_fixtures();
    parent.parent_span_is_remote = true;
    parent.parent_span_id = opentelemetry::trace::SpanId::from_bytes([1; 8]);
    let processor =
        DatabaseSpanProcessor::new(SimpleSpanProcessor::new(InMemorySpanExporter::default()));
    processor.on_end(query);
    processor.on_end(parent);
    assert!(processor.pending.lock().unwrap().is_empty());
    processor
        .shutdown_with_timeout(Duration::from_secs(1))
        .unwrap();
}

#[test]
fn shutdown_clears_summaries_for_operations_that_have_not_closed() {
    let (query, _) = span_fixtures();
    let processor =
        DatabaseSpanProcessor::new(SimpleSpanProcessor::new(InMemorySpanExporter::default()));
    processor.on_end(query);
    assert_eq!(processor.pending.lock().unwrap().len(), 1);
    processor
        .shutdown_with_timeout(Duration::from_secs(1))
        .unwrap();
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
    query.set_attribute(KeyValue::new(
        "db.statement",
        "SELECT id FROM users WHERE id = $1",
    ));
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
