import { config } from "./config";

export function activitySourceFileUrl(id: number | string) {
  return `${config.API_URL.replace(/\/$/, "")}/activities/${encodeURIComponent(
    String(id),
  )}/source-file`;
}
