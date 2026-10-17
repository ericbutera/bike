use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SimpleSpanProcessor, SpanData};
use std::future::Future;
use tracing::{instrument::WithSubscriber, Instrument};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Layer};

pub(crate) struct CapturedTrace {
    provider: SdkTracerProvider,
    exporter: InMemorySpanExporter,
}

impl CapturedTrace {
    pub(crate) fn spans(&self) -> Vec<SpanData> {
        self.provider.force_flush().unwrap();
        self.exporter.get_finished_spans().unwrap()
    }
}

pub(crate) async fn capture(future: impl Future<Output = ()>) -> CapturedTrace {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_span_processor(crate::observability::database_spans::DatabaseSpanProcessor::new(
            SimpleSpanProcessor::new(exporter.clone()),
        ))
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::new("info,sea_orm=trace,sqlx=warn"))
        .with(tracing_subscriber::fmt::layer().json().with_writer(std::io::sink))
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(provider.tracer("workflow-unit"))
                .with_filter(tracing_subscriber::filter::dynamic_filter_fn(|metadata, context| {
                    crate::observability::should_export_otel_span(
                        metadata.name(),
                        metadata.target(),
                        context.lookup_current().is_some(),
                    )
                })),
        );
    async {
        // Other tests may first register ORM callsites without a subscriber.
        tracing::callsite::rebuild_interest_cache();
        future.instrument(tracing::info_span!("request")).await;
    }
    .with_subscriber(subscriber)
    .await;
    CapturedTrace { provider, exporter }
}

pub(crate) fn stage(trace: &CapturedTrace, name: &str) -> SpanData {
    let spans = trace.spans();
    let request = spans.iter().find(|span| span.name == "request").unwrap();
    let stage = spans.iter().find(|span| span.name == name).unwrap();
    assert_eq!(stage.parent_span_id, request.span_context.span_id());
    assert_eq!(stage.span_context.trace_id(), request.span_context.trace_id());
    let queries: Vec<_> = spans.iter().filter(|span| span.name.starts_with("sea_orm.")).collect();
    assert!(
        !queries.is_empty(),
        "Expected database work below {name}; captured: {:?}",
        spans.iter().map(|span| &span.name).collect::<Vec<_>>()
    );
    for query in queries {
        assert!(query.start_time >= stage.start_time);
        assert!(query.end_time <= stage.end_time);
        let mut parent = query.parent_span_id;
        while parent != stage.span_context.span_id() {
            parent = spans
                .iter()
                .find(|span| span.span_context.span_id() == parent)
                .unwrap_or_else(|| panic!("{} is outside {name}", query.name))
                .parent_span_id;
        }
    }
    stage.clone()
}

pub(crate) fn attribute<'a>(span: &'a SpanData, key: &str) -> &'a opentelemetry::Value {
    &span
        .attributes
        .iter()
        .find(|field| field.key.as_str() == key)
        .unwrap_or_else(|| panic!("Missing {key} on {}", span.name))
        .value
}
