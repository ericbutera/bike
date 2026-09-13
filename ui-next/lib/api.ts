import {
  createClient,
  createFetchClient,
  fetchWithCredentials,
} from "@ericbutera/kaleido";
import { config } from "./config";
import { headersWithBrowserRequestContext } from "./trace-context";

const fetchWithRequestContext: typeof fetch = (input, init) =>
  fetchWithCredentials(input, {
    ...(init ?? {}),
    headers: headersWithBrowserRequestContext(init?.headers),
  });

export function createApiClient() {
  return createClient<any>(
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
    apiClient = createClient<any>(
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
