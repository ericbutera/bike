import { getServerConfig } from "./config";

export const SYNTHETIC_AUTH_HEADER = "x-bike-synthetic-key";

export function syntheticRequestHeaders(
  headers: Headers,
): Record<string, string> | null {
  const key = headers.get(SYNTHETIC_AUTH_HEADER);
  if (!key) return {};
  // Next adds X-Forwarded-For even to direct ClusterIP requests. The public
  // ingress strips this credential; do not classify Next's own header as public.
  if (headers.has("forwarded")) {
    return null;
  }
  return { [SYNTHETIC_AUTH_HEADER]: key };
}

export function apiBaseUrls() {
  const configured = process.env.INTERNAL_API_URL?.trim();
  if (configured) return [configured.replace(/\/$/, "")];
  const url = new URL(getServerConfig().API_URL);
  if (url.hostname === "localhost" || url.hostname === "127.0.0.1") {
    const publicUrl = url.toString().replace(/\/$/, "");
    url.hostname = "api";
    return [url.toString().replace(/\/$/, ""), publicUrl];
  }
  return [url.toString().replace(/\/$/, "")];
}
