import { createClient, createFetchClient } from "@ericbutera/kaleido";

export const API_URL =
  process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3000/api";

const fetchClient = createFetchClient({ baseUrl: API_URL });
export const $api = createClient<any>(fetchClient);
