import { context, propagation, type TextMapSetter } from "@opentelemetry/api";

const TRACE_CONTEXT_HEADERS = ["traceparent", "tracestate", "baggage"] as const;

const headerSetter: TextMapSetter<Headers> = {
  set(headers, key, value) {
    headers.set(key, value);
  },
};

export function headersWithTraceContext(
  headers?: HeadersInit,
  fallbackHeaders?: Headers,
) {
  const nextHeaders = new Headers(headers);

  propagation.inject(context.active(), nextHeaders, headerSetter);

  for (const header of TRACE_CONTEXT_HEADERS) {
    if (nextHeaders.has(header)) {
      continue;
    }

    const value = fallbackHeaders?.get(header);
    if (value) {
      nextHeaders.set(header, value);
    }
  }

  return nextHeaders;
}
