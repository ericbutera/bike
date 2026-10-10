package observability

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"log/slog"
	"testing"

	"github.com/stretchr/testify/require"
	"go.opentelemetry.io/otel"
	sdktrace "go.opentelemetry.io/otel/sdk/trace"
	"go.opentelemetry.io/otel/sdk/trace/tracetest"
)

func TestTracingExportSelection(t *testing.T) {
	for _, test := range []struct {
		config   tracingConfig
		expected bool
	}{
		{tracingConfig{Exporters: "otlp"}, true}, {tracingConfig{Exporters: " NONE, OTLP "}, true},
		{tracingConfig{Exporters: "none"}, false}, {tracingConfig{Disabled: true, Exporters: "otlp"}, false},
	} {
		require.Equal(t, test.expected, test.config.exportsOTLP())
	}
	previous := otel.GetTracerProvider()
	propagator := otel.GetTextMapPropagator()
	logger := slog.Default()
	t.Cleanup(func() {
		otel.SetTracerProvider(previous)
		otel.SetTextMapPropagator(propagator)
		slog.SetDefault(logger)
	})
	t.Setenv("OTEL_TRACES_EXPORTER", "none")
	shutdown, err := InitTracing(context.Background())
	require.NoError(t, err)
	_, span := otel.Tracer("fixture").Start(context.Background(), "export-disabled")
	require.True(t, span.IsRecording())
	require.True(t, span.SpanContext().IsValid())
	span.End()
	require.NoError(t, shutdown(context.Background()))
	t.Setenv("OTEL_SDK_DISABLED", "true")
	shutdown, err = InitTracing(context.Background())
	require.NoError(t, err)
	_, span = otel.Tracer("fixture").Start(context.Background(), "sdk-disabled")
	require.False(t, span.IsRecording())
	require.False(t, span.SpanContext().IsValid())
	span.End()
	require.NoError(t, shutdown(context.Background()))
	t.Setenv("OTEL_SDK_DISABLED", "invalid")
	_, err = InitTracing(context.Background())
	require.Error(t, err)
}

func TestErrorLogsContainTraceIdentity(t *testing.T) {
	exporter := tracetest.NewInMemoryExporter()
	provider := sdktrace.NewTracerProvider(sdktrace.WithSyncer(exporter))
	ctx, span := provider.Tracer("fixture").Start(context.Background(), "render")
	var output bytes.Buffer
	logger := slog.Default()
	t.Cleanup(func() { slog.SetDefault(logger) })
	slog.SetDefault(slog.New(slog.NewJSONHandler(&output, nil)))
	Error(ctx, "fixture error", errors.New("browser failed"), "browser_render")
	span.End()
	var record map[string]any
	require.NoError(t, json.Unmarshal(output.Bytes(), &record))
	require.Equal(t, "browser_render", record["stage"])
	require.Equal(t, span.SpanContext().TraceID().String(), record["trace_id"])
	require.Len(t, exporter.GetSpans(), 1)
	require.NoError(t, provider.Shutdown(context.Background()))
}
