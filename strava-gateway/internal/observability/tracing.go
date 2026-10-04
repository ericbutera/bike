package observability

import (
	"context"
	"os"
	"strings"
	"time"

	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/exporters/otlp/otlptrace/otlptracehttp"
	"go.opentelemetry.io/otel/propagation"
	"go.opentelemetry.io/otel/sdk/resource"
	sdktrace "go.opentelemetry.io/otel/sdk/trace"
	"go.opentelemetry.io/otel/trace"
)

func InitTracing(ctx context.Context, serviceName string) (func(context.Context) error, error) {
	otel.SetTextMapPropagator(propagation.NewCompositeTextMapPropagator(
		propagation.TraceContext{}, propagation.Baggage{},
	))
	attributes := []attribute.KeyValue{
		attribute.String("service.name", serviceName),
		attribute.String("service.version", envOr("BIKE_VERSION", "unknown")),
		attribute.String("deployment.environment.name", envOr("APP_ENV", "development")),
	}
	resource := resource.NewSchemaless(attributes...)
	options := []sdktrace.TracerProviderOption{
		sdktrace.WithResource(resource),
		// Tail sampling needs successful spans and unsampled remote parents too.
		sdktrace.WithSampler(sdktrace.AlwaysSample()),
	}
	if exporterEnabled() {
		exporter, err := otlptracehttp.New(ctx)
		if err != nil {
			return nil, err
		}
		options = append(options, sdktrace.WithBatcher(exporter,
			sdktrace.WithBatchTimeout(2*time.Second),
			sdktrace.WithMaxExportBatchSize(256),
		))
	}
	provider := sdktrace.NewTracerProvider(options...)
	otel.SetTracerProvider(provider)
	return provider.Shutdown, nil
}

func exporterEnabled() bool {
	if strings.EqualFold(os.Getenv("OTEL_SDK_DISABLED"), "true") || strings.EqualFold(os.Getenv("OTEL_TRACES_EXPORTER"), "none") {
		return false
	}
	if configured := strings.TrimSpace(os.Getenv("OTEL_TRACES_EXPORTER")); configured != "" {
		for _, exporter := range strings.Split(configured, ",") {
			if strings.EqualFold(strings.TrimSpace(exporter), "otlp") {
				return true
			}
		}
		return false
	}
	return os.Getenv("OTEL_EXPORTER_OTLP_ENDPOINT") != "" || os.Getenv("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT") != ""
}

func envOr(name, fallback string) string {
	if value := strings.TrimSpace(os.Getenv(name)); value != "" {
		return value
	}
	return fallback
}

func StartJobSpan(ctx context.Context, traceparent, tracestate *string, name string, attrs ...attribute.KeyValue) (context.Context, trace.Span) {
	carrier := propagation.MapCarrier{}
	if traceparent != nil {
		carrier.Set("traceparent", *traceparent)
	}
	if tracestate != nil {
		carrier.Set("tracestate", *tracestate)
	}
	parent := otel.GetTextMapPropagator().Extract(context.Background(), carrier)
	parentContext := trace.SpanContextFromContext(parent)
	options := []trace.SpanStartOption{trace.WithAttributes(attrs...)}
	options = append(options, trace.WithSpanKind(trace.SpanKindConsumer))
	if parentContext.IsValid() {
		options = append(options, trace.WithLinks(trace.Link{SpanContext: parentContext}))
	}
	return otel.Tracer("bike.strava.gateway").Start(ctx, name, options...)
}
