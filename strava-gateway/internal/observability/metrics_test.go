package observability

import (
	"context"
	"encoding/json"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	sdktrace "go.opentelemetry.io/otel/sdk/trace"
)

func TestHTTPMiddlewareLogsTraceIDsWithoutLoggingQueryValues(t *testing.T) {
	previousLogger := slog.Default()
	var logOutput strings.Builder
	slog.SetDefault(NewLogger("bike-strava-gateway", &logOutput))
	t.Cleanup(func() { slog.SetDefault(previousLogger) })

	provider := sdktrace.NewTracerProvider()
	t.Cleanup(func() { _ = provider.Shutdown(context.Background()) })
	ctx, span := provider.Tracer("test").Start(context.Background(), "gateway callback")
	defer span.End()

	request := httptest.NewRequest(http.MethodGet, "/oauth/gateway/callback?code=private-value", nil)
	request.Pattern = "GET /oauth/gateway/callback"
	request = request.WithContext(ctx)
	metrics := NewMetrics()
	for _, status := range []int{http.StatusForbidden, http.StatusInternalServerError} {
		logOutput.Reset()
		recorder := httptest.NewRecorder()
		handler := metrics.HTTPMiddleware(http.HandlerFunc(func(writer http.ResponseWriter, _ *http.Request) {
			writer.WriteHeader(status)
		}))
		handler.ServeHTTP(recorder, request)

		var fields map[string]any
		if err := json.Unmarshal([]byte(logOutput.String()), &fields); err != nil {
			t.Fatalf("decode structured request log: %v", err)
		}
		if got := fields["trace_id"]; got != span.SpanContext().TraceID().String() {
			t.Fatalf("logged trace_id = %v", got)
		}
		if got := fields["span_id"]; got != span.SpanContext().SpanID().String() {
			t.Fatalf("logged span_id = %v", got)
		}
		if got := fields["route"]; got != "GET /oauth/gateway/callback" {
			t.Fatalf("logged route = %v", got)
		}
		if got := fields["status_code"]; got != float64(status) {
			t.Fatalf("logged status_code = %v, want %d", got, status)
		}
		if strings.Contains(logOutput.String(), "private-value") {
			t.Fatal("request log contains an OAuth query value")
		}
	}
}
