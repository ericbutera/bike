export function errorLogRecord(message, error, fields = {}, spanContext) {
  const record = {
    level: "error",
    message,
    ...fields,
    error_type: error instanceof Error ? error.name : typeof error,
  };
  if (spanContext?.traceId && spanContext?.spanId) {
    record.trace_id = spanContext.traceId;
    record.span_id = spanContext.spanId;
  }
  return record;
}
