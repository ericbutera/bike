package observability

import (
	"io"
	"log/slog"

	"go.opentelemetry.io/otel/trace"
)

func NewLogger(service string, output io.Writer) *slog.Logger {
	if output == nil {
		output = io.Discard
	}

	return slog.New(slog.NewJSONHandler(output, nil)).With("service", service)
}

func WithSpanFields(logger *slog.Logger, span trace.Span) *slog.Logger {
	spanContext := span.SpanContext()
	if !spanContext.IsValid() {
		return logger
	}

	return logger.With(
		"trace_id", spanContext.TraceID().String(),
		"span_id", spanContext.SpanID().String(),
	)
}
