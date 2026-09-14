const TRACE_CONTEXT_HEADERS = ["traceparent", "tracestate", "baggage"] as const;
const REQUEST_CONTEXT_HEADERS = ["x-request-id"] as const;
const CONTEXT_HEADERS = [
  ...TRACE_CONTEXT_HEADERS,
  ...REQUEST_CONTEXT_HEADERS,
] as const;

export type RequestContextHeaders = Partial<
  Record<(typeof CONTEXT_HEADERS)[number], string>
>;

declare global {
  interface Window {
    __REQUEST_CONTEXT__?: RequestContextHeaders;
  }
}

export function headersWithTraceContext(
  headers?: HeadersInit,
  fallbackHeaders?: Headers,
) {
  const nextHeaders = new Headers(headers);

  for (const header of CONTEXT_HEADERS) {
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

export function requestContextFromHeaders(
  headers: Headers,
): RequestContextHeaders {
  const contextHeaders: RequestContextHeaders = {};

  for (const header of CONTEXT_HEADERS) {
    const value = headers.get(header);
    if (value) {
      contextHeaders[header] = value;
    }
  }

  return contextHeaders;
}

export function headersWithBrowserRequestContext(headers?: HeadersInit) {
  const nextHeaders = new Headers(headers);
  const requestContext =
    typeof window === "undefined" ? undefined : window.__REQUEST_CONTEXT__;

  if (!requestContext) {
    return nextHeaders;
  }

  for (const header of CONTEXT_HEADERS) {
    const value = requestContext[header];
    if (value && !nextHeaders.has(header)) {
      nextHeaders.set(header, value);
    }
  }

  return nextHeaders;
}
