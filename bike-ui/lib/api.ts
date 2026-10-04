import createFetchClient from "openapi-fetch";
import createQueryClient from "openapi-react-query";
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

const fetchWithRequestContext: typeof fetch = (input, init) => {
  return fetchWithCredentials(requestWithContext(input, init));
};

export function createApiClient() {
  return createQueryClient<any>(
    createFetchClient({
      baseUrl: config.API_URL,
      fetch: fetchWithRequestContext,
    }),
  );
}

export function createTypedApiClient() {
  return createQueryClient<paths>(
    createFetchClient({
      baseUrl: config.API_URL,
      fetch: fetchWithRequestContext,
    }),
  );
}

export function handleApiError(error: unknown) {
  if (typeof error === "object" && error !== null && "response" in error) {
    const response = (error as { response?: { data?: unknown } }).response;
    const data = response?.data;
    if (typeof data === "object" && data !== null) {
      const record = data as Record<string, unknown>;
      return {
        message:
          typeof record.message === "string" ? record.message : undefined,
        errors: record.errors as Record<string, string[]> | undefined,
      };
    }
  }

  return {
    message: error instanceof Error ? error.message : "Request failed",
    errors: undefined,
  };
}

export async function fetchWithCredentials(
  input: RequestInfo | URL,
  init?: RequestInit,
) {
  const response = await fetch(input, {
    ...(init ?? {}),
    credentials: "include",
  });
  let data: unknown;
  if (response.headers.get("content-type")?.includes("application/json")) {
    try {
      data = await response.clone().json();
    } catch {
      data = undefined;
    }
  }
  if (!response.ok) {
    const message =
      typeof data === "object" &&
      data !== null &&
      "message" in data &&
      typeof (data as { message?: unknown }).message === "string"
        ? (data as { message: string }).message
        : response.statusText || "Request failed";
    const error = new Error(message) as Error & {
      response?: { status: number; data?: unknown };
    };
    error.response = { status: response.status, data };
    throw error;
  }
  return response;
}

let apiClient: ReturnType<typeof createApiClient> | null = null;
let apiBaseUrl: string | null = null;

function getApiClient() {
  const baseUrl = config.API_URL;

  if (!apiClient || apiBaseUrl !== baseUrl) {
    apiClient = createQueryClient<any>(
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

let typedApiClient: ReturnType<typeof createTypedApiClient> | null = null;
let typedApiBaseUrl: string | null = null;

function getTypedApiClient() {
  const baseUrl = config.API_URL;

  if (!typedApiClient || typedApiBaseUrl !== baseUrl) {
    typedApiClient = createTypedApiClient();
    typedApiBaseUrl = baseUrl;
  }

  return typedApiClient;
}

export const $typedApi = new Proxy(
  {} as ReturnType<typeof createTypedApiClient>,
  {
    get(_target, prop, receiver) {
      const client = getTypedApiClient();
      const value = Reflect.get(client as object, prop, receiver);

      return typeof value === "function" ? value.bind(client) : value;
    },
  },
);
