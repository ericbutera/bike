import { createClient, createFetchClient } from "@ericbutera/kaleido";
import { config } from "./config";

export function createApiClient() {
  return createClient<any>(createFetchClient({ baseUrl: config.API_URL }));
}

export const $api = createApiClient();
