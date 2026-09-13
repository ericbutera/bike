use axum::http::HeaderMap;
use opentelemetry::global;
use opentelemetry::propagation::{Extractor, Injector};
use opentelemetry::trace::{TraceContextExt, TracerProvider as _};
use opentelemetry::{Context, KeyValue};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use std::collections::HashMap;
use tracing::field;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::filter::dynamic_filter_fn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer as _;

pub type TraceContextCarrier = HashMap<String, String>;

pub struct ObservabilityGuard {
    tracer_provider: Option<SdkTracerProvider>,
}

impl Drop for ObservabilityGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.tracer_provider.take() {
            if let Err(errors) = provider.shutdown() {
                eprintln!("failed to shut down OpenTelemetry tracer provider: {errors:?}");
            }
        }
    }
}

pub fn init_observability(service_name: &'static str) -> ObservabilityGuard {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true);
    let registry = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt_layer);

    match build_tracer_provider(service_name) {
        Ok(provider) => {
            let tracer = provider.tracer(service_name);
            global::set_tracer_provider(provider.clone());
            let otel_layer = tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(dynamic_filter_fn(|metadata, ctx| {
                    should_export_otel_span(metadata.target(), ctx.lookup_current().is_some())
                }));
            if registry.with(otel_layer).try_init().is_err() {
                return ObservabilityGuard {
                    tracer_provider: Some(provider),
                };
            }
            ObservabilityGuard {
                tracer_provider: Some(provider),
            }
        }
        Err(error) => {
            let _ = registry.try_init();
            tracing::warn!(
                error = %error,
                service_name,
                "OpenTelemetry OTLP tracing exporter disabled"
            );
            ObservabilityGuard {
                tracer_provider: None,
            }
        }
    }
}

fn build_tracer_provider(
    service_name: &'static str,
) -> Result<SdkTracerProvider, Box<dyn std::error::Error + Send + Sync>> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .build()?;
    let resource = Resource::builder()
        .with_service_name(service_name)
        .with_attributes(vec![KeyValue::new(
            "deployment.environment",
            std::env::var("APP_ENV")
                .or_else(|_| std::env::var("ENVIRONMENT"))
                .unwrap_or_else(|_| "development".to_string()),
        )])
        .build();

    Ok(SdkTracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build())
}

fn should_export_otel_span(target: &str, has_exported_parent: bool) -> bool {
    has_exported_parent || is_application_trace_target(target)
}

fn is_application_trace_target(target: &str) -> bool {
    target == "api"
        || target.starts_with("api::")
        || target == "worker"
        || target.starts_with("worker::")
        || target == "kaleido"
        || target.starts_with("kaleido::")
}

pub fn inject_current_trace_context() -> Option<TraceContextCarrier> {
    let context = tracing::Span::current().context();
    let span_context = context.span().span_context().clone();
    if !span_context.is_valid() {
        return None;
    }

    let mut carrier = TraceContextCarrier::new();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&context, &mut TraceContextInjector(&mut carrier));
    });
    (!carrier.is_empty()).then_some(carrier)
}

pub fn set_span_parent_from_carrier(span: &tracing::Span, carrier: Option<&TraceContextCarrier>) {
    let Some(carrier) = carrier else {
        return;
    };

    let parent_context = global::get_text_map_propagator(|propagator| {
        propagator.extract(&TraceContextExtractor(carrier))
    });
    let _ = span.set_parent(parent_context);
}

pub fn set_span_parent_from_headers(span: &tracing::Span, headers: &HeaderMap) {
    let parent_context = global::get_text_map_propagator(|propagator| {
        propagator.extract(&TraceHeaderExtractor(headers))
    });
    let _ = span.set_parent(parent_context);
}

pub fn record_span_trace_context(span: &tracing::Span) {
    let context = span.context();
    record_context_fields(span, &context);
}

pub fn record_current_trace_context() {
    let span = tracing::Span::current();
    let context = span.context();
    record_context_fields(&span, &context);
}

fn record_context_fields(span: &tracing::Span, context: &Context) {
    let span_context = context.span().span_context().clone();
    if !span_context.is_valid() {
        return;
    }

    span.record("trace_id", field::display(span_context.trace_id()));
    span.record("span_id", field::display(span_context.span_id()));
}

struct TraceContextInjector<'a>(&'a mut TraceContextCarrier);

impl Injector for TraceContextInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

struct TraceContextExtractor<'a>(&'a TraceContextCarrier);

impl Extractor for TraceContextExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }
}

struct TraceHeaderExtractor<'a>(&'a HeaderMap);

impl Extractor for TraceHeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.to_str().ok()
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|key| key.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::should_export_otel_span;

    #[test]
    fn otel_filter_allows_application_roots() {
        assert!(should_export_otel_span("api", false));
        assert!(should_export_otel_span(
            "api::controllers::activities",
            false
        ));
        assert!(should_export_otel_span("worker", false));
        assert!(should_export_otel_span(
            "worker::tasks::processors::strava_sync",
            false
        ));
        assert!(should_export_otel_span(
            "kaleido::background_jobs::worker",
            false
        ));
    }

    #[test]
    fn otel_filter_drops_parentless_library_roots() {
        assert!(!should_export_otel_span("sea_orm", false));
        assert!(!should_export_otel_span(
            "sea_orm::database::db_connection",
            false
        ));
        assert!(!should_export_otel_span("sqlx::query", false));
        assert!(!should_export_otel_span("hyper::proto::h1", false));
    }

    #[test]
    fn otel_filter_keeps_library_spans_under_exported_parent() {
        assert!(should_export_otel_span("sea_orm", true));
        assert!(should_export_otel_span(
            "sea_orm::database::db_connection",
            true
        ));
        assert!(should_export_otel_span("sqlx::query", true));
        assert!(should_export_otel_span("hyper::proto::h1", true));
    }
}
