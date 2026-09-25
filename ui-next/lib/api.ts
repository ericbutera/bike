import {
  createClient,
  createFetchClient,
  fetchWithCredentials,
} from "@ericbutera/kaleido";
import { config } from "./config";
import type { paths } from "./openapi/react-query/api";
import { headersWithBrowserRequestContext } from "./trace-context";

function shouldDefaultToJson(
  body: BodyInit | ReadableStream | null | undefined,
) {
  if (body == null) {
    return false;
  }

  if (
    (typeof FormData !== "undefined" && body instanceof FormData) ||
    (typeof URLSearchParams !== "undefined" &&
      body instanceof URLSearchParams) ||
    (typeof Blob !== "undefined" && body instanceof Blob)
  ) {
    return false;
  }

  return true;
}

function headersForRequest(input: RequestInfo | URL, init?: RequestInit) {
  const headers = new Headers(
    input instanceof Request ? input.headers : undefined,
  );

  if (init?.headers) {
    new Headers(init.headers).forEach((value, key) => {
      headers.set(key, value);
    });
  }

  const headersWithContext = headersWithBrowserRequestContext(headers);
  if (
    shouldDefaultToJson(init?.body) &&
    !headersWithContext.has("Content-Type")
  ) {
    headersWithContext.set("Content-Type", "application/json");
  }

  return headersWithContext;
}

function requestWithContext(input: RequestInfo | URL, init?: RequestInit) {
  return new Request(input, {
    ...(init ?? {}),
    headers: headersForRequest(input, init),
  });
}

export function normalizeApiError(error: unknown): Error | null {
  if (!error) {
    return null;
  }
  if (error instanceof Error) {
    return error;
  }
  if (typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") {
      return new Error(message);
    }
  }
  return new Error("Request failed");
}

const fetchWithRequestContext: typeof fetch = (input, init) => {
  return fetchWithCredentials(requestWithContext(input, init));
};

export function createApiClient() {
  return createClient<paths>(
    createFetchClient({
      baseUrl: config.API_URL,
      fetch: fetchWithRequestContext,
    }),
  );
}

let apiClient: ReturnType<typeof createApiClient> | null = null;
let apiBaseUrl: string | null = null;

function getApiClient() {
  const baseUrl = config.API_URL;

  if (!apiClient || apiBaseUrl !== baseUrl) {
    apiClient = createClient<paths>(
      createFetchClient({
        baseUrl,
        fetch: fetchWithRequestContext,
      }),
    );
    apiBaseUrl = baseUrl;
  }

  return apiClient;
}

export const $api = new Proxy({} as ReturnType<typeof createApiClient>, {
  get(_target, prop, receiver) {
    const client = getApiClient();
    const value = Reflect.get(client as object, prop, receiver);

    return typeof value === "function" ? value.bind(client) : value;
  },
});
