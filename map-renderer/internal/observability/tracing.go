package observability

import (
	"context"
	"log/slog"
	"os"
	"strings"

	"github.com/caarlos0/env/v11"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/codes"
	"go.opentelemetry.io/otel/exporters/otlp/otlptrace/otlptracehttp"
	"go.opentelemetry.io/otel/propagation"
	"go.opentelemetry.io/otel/sdk/resource"
	sdktrace "go.opentelemetry.io/otel/sdk/trace"
	"go.opentelemetry.io/otel/trace"
	"go.opentelemetry.io/otel/trace/noop"
)

type tracingConfig struct {
	Service     string `env:"OTEL_SERVICE_NAME" envDefault:"bike-map-renderer"`
	Version     string `env:"BIKE_VERSION" envDefault:"unknown"`
	Environment string `env:"APP_ENV" envDefault:"development"`
	Disabled    bool   `env:"OTEL_SDK_DISABLED"`
	Exporters   string `env:"OTEL_TRACES_EXPORTER" envDefault:"otlp"`
}

func InitTracing(ctx context.Context) (func(context.Context) error, error) {
	config, err := env.ParseAs[tracingConfig]()
	if err != nil {
		return nil, err
	}
	otel.SetTextMapPropagator(propagation.NewCompositeTextMapPropagator(propagation.TraceContext{}, propagation.Baggage{}))
	slog.SetDefault(slog.New(slog.NewJSONHandler(os.Stderr, nil)).With("service", config.Service))
	if config.Disabled {
		otel.SetTracerProvider(noop.NewTracerProvider())
		return func(context.Context) error { return nil }, nil
	}
	options := []sdktrace.TracerProviderOption{
		sdktrace.WithResource(resource.NewSchemaless(
			attribute.String("service.name", config.Service),
			attribute.String("service.version", config.Version),
			attribute.String("deployment.environment.name", config.Environment))),
		sdktrace.WithSampler(sdktrace.AlwaysSample()),
	}
	if config.exportsOTLP() {
		exporter, err := otlptracehttp.New(ctx)
		if err != nil {
			return nil, err
		}
		options = append(options, sdktrace.WithBatcher(exporter))
	}
	provider := sdktrace.NewTracerProvider(options...)
	otel.SetTracerProvider(provider)
	return provider.Shutdown, nil
}

func (c tracingConfig) exportsOTLP() bool {
	if c.Disabled {
		return false
	}
	for _, name := range strings.Split(c.Exporters, ",") {
		if strings.EqualFold(strings.TrimSpace(name), "otlp") {
			return true
		}
	}
	return false
}

func Error(ctx context.Context, message string, err error, stage string) {
	span := trace.SpanFromContext(ctx)
	span.RecordError(err)
	span.SetStatus(codes.Error, err.Error())
	fields := []any{"error", err.Error(), "stage", stage}
	if sc := span.SpanContext(); sc.IsValid() {
		fields = append(fields, "trace_id", sc.TraceID().String(), "span_id", sc.SpanID().String())
	}
	slog.ErrorContext(ctx, message, fields...)
}
