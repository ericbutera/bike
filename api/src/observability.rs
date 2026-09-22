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

enum TraceExporterConfig {
    Disabled { reason: &'static str },
    Otlp { traces_endpoint: String },
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

    match trace_exporter_config() {
        TraceExporterConfig::Disabled { reason } => init_logs_only(service_name, reason),
        TraceExporterConfig::Otlp { traces_endpoint } => {
            init_otel_or_log_fallback(service_name, &traces_endpoint)
        }
    }
}

fn init_logs_only(service_name: &'static str, reason: &'static str) -> ObservabilityGuard {
    let _ = init_log_subscriber();
    tracing::info!(
        service_name,
        reason,
        "OpenTelemetry OTLP tracing exporter disabled"
    );
    ObservabilityGuard {
        tracer_provider: None,
    }
}

fn init_otel_or_log_fallback(
    service_name: &'static str,
    traces_endpoint: &str,
) -> ObservabilityGuard {
    match init_otel_subscriber(service_name, traces_endpoint) {
        Ok(guard) => guard,
        Err(error) => {
            let _ = init_log_subscriber();
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

fn init_log_subscriber() -> Result<(), tracing_subscriber::util::TryInitError> {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt_layer)
        .try_init()
}

fn init_otel_subscriber(
    service_name: &'static str,
    traces_endpoint: &str,
) -> Result<ObservabilityGuard, Box<dyn std::error::Error + Send + Sync>> {
    let provider = build_tracer_provider(service_name, traces_endpoint)?;
    let tracer = provider.tracer(service_name);
    global::set_tracer_provider(provider.clone());

    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true);
    let otel_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_filter(dynamic_filter_fn(|metadata, ctx| {
            should_export_otel_span(
                metadata.name(),
                metadata.target(),
                ctx.lookup_current().is_some(),
            )
        }));
    if tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt_layer)
        .with(otel_layer)
        .try_init()
        .is_err()
    {
        return Ok(ObservabilityGuard {
            tracer_provider: Some(provider),
        });
    }

    tracing::info!(
        service_name,
        otel_traces_endpoint = %traces_endpoint,
        otel_protocol = %otel_protocol_label(),
        "OpenTelemetry OTLP tracing exporter enabled"
    );
    Ok(ObservabilityGuard {
        tracer_provider: Some(provider),
    })
}

fn build_tracer_provider(
    service_name: &'static str,
    traces_endpoint: &str,
) -> Result<SdkTracerProvider, Box<dyn std::error::Error + Send + Sync>> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(traces_endpoint)
        .with_protocol(otel_protocol())
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

fn trace_exporter_config() -> TraceExporterConfig {
    trace_exporter_config_from_values(
        std::env::var("OTEL_SDK_DISABLED").ok().as_deref(),
        std::env::var("OTEL_TRACES_EXPORTER").ok().as_deref(),
        std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
            .ok()
            .as_deref(),
        std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok().as_deref(),
    )
}

fn trace_exporter_config_from_values(
    sdk_disabled: Option<&str>,
    traces_exporter: Option<&str>,
    traces_endpoint: Option<&str>,
    otlp_endpoint: Option<&str>,
) -> TraceExporterConfig {
    if env_flag_enabled(sdk_disabled) {
        return TraceExporterConfig::Disabled {
            reason: "OTEL_SDK_DISABLED=true",
        };
    }

    if let Some(exporter) = traces_exporter.and_then(non_empty_env_value) {
        let exporters: Vec<String> = exporter
            .split(',')
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect();

        if exporters.iter().any(|value| value == "none") {
            return TraceExporterConfig::Disabled {
                reason: "OTEL_TRACES_EXPORTER=none",
            };
        }

        if !exporters.iter().any(|value| value == "otlp") {
            return TraceExporterConfig::Disabled {
                reason: "OTEL_TRACES_EXPORTER does not include otlp",
            };
        }

        return TraceExporterConfig::Otlp {
            traces_endpoint: configured_otel_traces_endpoint(traces_endpoint, otlp_endpoint)
                .unwrap_or_else(|| append_otel_traces_path("http://localhost:4318")),
        };
    }

    let Some(traces_endpoint) = configured_otel_traces_endpoint(traces_endpoint, otlp_endpoint)
    else {
        return TraceExporterConfig::Disabled {
            reason: "no OTLP endpoint configured",
        };
    };

    TraceExporterConfig::Otlp { traces_endpoint }
}

fn configured_otel_traces_endpoint(
    traces_endpoint: Option<&str>,
    otlp_endpoint: Option<&str>,
) -> Option<String> {
    if let Some(endpoint) = traces_endpoint.and_then(non_empty_env_value) {
        return Some(append_otel_traces_path(endpoint));
    }

    otlp_endpoint
        .and_then(non_empty_env_value)
        .map(append_otel_traces_path)
}

fn non_empty_env_value(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn env_flag_enabled(value: Option<&str>) -> bool {
    value
        .and_then(|value| non_empty_env_value(value).map(str::to_ascii_lowercase))
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes"))
}

fn append_otel_traces_path(endpoint: &str) -> String {
    let endpoint = endpoint.trim_end_matches('/');

    if endpoint.ends_with("/v1/traces") {
        endpoint.to_string()
    } else {
        format!("{endpoint}/v1/traces")
    }
}

fn otel_protocol() -> Protocol {
    Protocol::HttpBinary
}

fn otel_protocol_label() -> &'static str {
    "http/protobuf"
}

fn should_export_otel_span(name: &str, target: &str, has_exported_parent: bool) -> bool {
    if name == "background_task.poll" {
        return false;
    }

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
    use super::TraceExporterConfig;
    use super::{
        append_otel_traces_path, should_export_otel_span, trace_exporter_config_from_values,
    };

    #[test]
    fn otel_filter_allows_application_roots() {
        assert!(should_export_otel_span("request", "api", false));
        assert!(should_export_otel_span(
            "activity.list",
            "api::controllers::activities",
            false,
        ));
        assert!(should_export_otel_span("worker.run", "worker", false));
        assert!(should_export_otel_span(
            "worker.strava_sync",
            "worker::tasks::processors::strava_sync",
            false,
        ));
        assert!(should_export_otel_span(
            "background_task.process",
            "kaleido::background_jobs::worker",
            false,
        ));
    }

    #[test]
    fn otel_filter_drops_parentless_library_roots() {
        assert!(!should_export_otel_span("query", "sea_orm", false,));
        assert!(!should_export_otel_span(
            "query",
            "sea_orm::database::db_connection",
            false,
        ));
        assert!(!should_export_otel_span("query", "sqlx::query", false));
        assert!(!should_export_otel_span(
            "connection",
            "hyper::proto::h1",
            false,
        ));
    }

    #[test]
    fn otel_filter_drops_idle_worker_poll_spans() {
        assert!(!should_export_otel_span(
            "background_task.poll",
            "kaleido::background_jobs::worker::task_worker",
            false,
        ));
        assert!(!should_export_otel_span(
            "background_task.poll",
            "kaleido::background_jobs::worker::task_worker",
            true,
        ));
    }

    #[test]
    fn otel_filter_keeps_library_spans_under_exported_parent() {
        assert!(should_export_otel_span("query", "sea_orm", true,));
        assert!(should_export_otel_span(
            "query",
            "sea_orm::database::db_connection",
            true,
        ));
        assert!(should_export_otel_span("query", "sqlx::query", true));
        assert!(should_export_otel_span(
            "connection",
            "hyper::proto::h1",
            true,
        ));
    }

    #[test]
    fn otel_traces_endpoint_appends_signal_path() {
        assert_eq!(
            append_otel_traces_path("http://tempo.observability.svc.cluster.local:4318"),
            "http://tempo.observability.svc.cluster.local:4318/v1/traces"
        );
        assert_eq!(
            append_otel_traces_path("http://tempo.observability.svc.cluster.local:4318/"),
            "http://tempo.observability.svc.cluster.local:4318/v1/traces"
        );
        assert_eq!(
            append_otel_traces_path("http://tempo.observability.svc.cluster.local:4318/v1/traces"),
            "http://tempo.observability.svc.cluster.local:4318/v1/traces"
        );
    }

    #[test]
    fn otel_exporter_is_disabled_without_explicit_endpoint_or_exporter() {
        assert!(matches!(
            trace_exporter_config_from_values(None, None, None, None),
            TraceExporterConfig::Disabled {
                reason: "no OTLP endpoint configured"
            }
        ));
    }

    #[test]
    fn otel_exporter_honors_standard_disable_flags() {
        assert!(matches!(
            trace_exporter_config_from_values(Some("true"), Some("otlp"), None, None),
            TraceExporterConfig::Disabled {
                reason: "OTEL_SDK_DISABLED=true"
            }
        ));
        assert!(matches!(
            trace_exporter_config_from_values(None, Some("none"), None, Some("http://jaeger:4318")),
            TraceExporterConfig::Disabled {
                reason: "OTEL_TRACES_EXPORTER=none"
            }
        ));
    }

    #[test]
    fn otel_exporter_uses_configured_endpoint_when_present() {
        let TraceExporterConfig::Otlp { traces_endpoint } =
            trace_exporter_config_from_values(None, None, None, Some("http://jaeger:4318"))
        else {
            panic!("expected OTLP exporter to be enabled");
        };

        assert_eq!(traces_endpoint, "http://jaeger:4318/v1/traces");
    }

    #[test]
    fn otel_exporter_uses_localhost_when_otlp_is_explicitly_requested() {
        let TraceExporterConfig::Otlp { traces_endpoint } =
            trace_exporter_config_from_values(None, Some("otlp"), None, None)
        else {
            panic!("expected OTLP exporter to be enabled");
        };

        assert_eq!(traces_endpoint, "http://localhost:4318/v1/traces");
    }
}
