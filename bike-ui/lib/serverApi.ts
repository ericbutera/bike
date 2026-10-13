import { getServerConfig } from "./config";

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
