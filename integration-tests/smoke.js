import http from "k6/http";
import { check, fail } from "k6";

export const options = {
  vus: 1,
  iterations: 1,
  thresholds: {
    checks: ["rate==1"],
    http_req_failed: ["rate==0"],
  },
};

export default function () {
  const api = (__ENV.BIKE_API_URL || "").replace(/\/$/, "");
  const ui = (__ENV.BIKE_UI_URL || "").replace(/\/$/, "");
  if (!api || !ui) fail("Set BIKE_API_URL (including /api) and BIKE_UI_URL");

  const health = http.get(`${api}/health`, { timeout: "10s", redirects: 0 });
  check(health, {
    "API is healthy": (response) =>
      response.status === 200 && response.json().status === "healthy",
  });
  const page = http.get(ui, { timeout: "10s", redirects: 0 });
  check(page, {
    "UI serves HTML": (response) =>
      response.status === 200 && response.body.includes("<html"),
  });
}
