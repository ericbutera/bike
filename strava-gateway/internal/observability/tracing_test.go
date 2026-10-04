package observability

import (
	"bytes"
	"context"
	"encoding/json"
	"testing"

	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/propagation"
	sdktrace "go.opentelemetry.io/otel/sdk/trace"
	"go.opentelemetry.io/otel/sdk/trace/tracetest"
	"go.opentelemetry.io/otel/trace"
)

func TestStartJobSpanLinksPersistedTraceContextAsConsumer(t *testing.T) {
	previousProvider := otel.GetTracerProvider()
	previousPropagator := otel.GetTextMapPropagator()
	recorder := tracetest.NewSpanRecorder()
	provider := sdktrace.NewTracerProvider(sdktrace.WithSpanProcessor(recorder))
	otel.SetTracerProvider(provider)
	otel.SetTextMapPropagator(propagation.TraceContext{})
	t.Cleanup(func() {
		_ = provider.Shutdown(context.Background())
		otel.SetTracerProvider(previousProvider)
		otel.SetTextMapPropagator(previousPropagator)
	})

	traceparent := "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-00"
	tracestate := "vendor=value"
	_, span := StartJobSpan(context.Background(), &traceparent, &tracestate, "gateway.site_delivery")
	span.End()

	ended := recorder.Ended()
	if len(ended) != 1 {
		t.Fatalf("ended spans = %d, want 1", len(ended))
	}
	if got := ended[0].SpanKind(); got != trace.SpanKindConsumer {
		t.Fatalf("job span kind = %s", got)
	}
	if got := ended[0].SpanContext().TraceID().String(); got == "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" {
		t.Fatalf("linked job unexpectedly continued producer trace %s", got)
	}
	links := ended[0].Links()
	if len(links) != 1 {
		t.Fatalf("job span links = %d, want 1", len(links))
	}
	if got := links[0].SpanContext.TraceID().String(); got != "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" {
		t.Fatalf("linked trace ID = %s", got)
	}
	if got := links[0].SpanContext.SpanID().String(); got != "bbbbbbbbbbbbbbbb" {
		t.Fatalf("linked span ID = %s", got)
	}
	if got := links[0].SpanContext.TraceState().String(); got != tracestate {
		t.Fatalf("linked trace state = %s", got)
	}

	var logOutput bytes.Buffer
	WithSpanFields(NewLogger("bike-strava-worker", &logOutput), span).Warn("job failed")
	var logFields map[string]any
	if err := json.Unmarshal(logOutput.Bytes(), &logFields); err != nil {
		t.Fatalf("decode correlated job log: %v", err)
	}
	if got := logFields["trace_id"]; got != ended[0].SpanContext().TraceID().String() {
		t.Fatalf("job log trace_id = %v", got)
	}
	if got := logFields["span_id"]; got != ended[0].SpanContext().SpanID().String() {
		t.Fatalf("job log span_id = %v", got)
	}
	if got := logFields["service"]; got != "bike-strava-worker" {
		t.Fatalf("job log service = %v", got)
	}
}
