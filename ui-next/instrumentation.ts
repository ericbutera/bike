import { registerOTel, type FetchInstrumentationConfig } from "@vercel/otel";

function tracePropagationUrls(): FetchInstrumentationConfig["propagateContextUrls"] {
  return Array.from(
    new Set(
      [process.env.INTERNAL_API_URL, process.env.API_URL, "http://api:3000/api"]
        .map((value) => value?.trim().replace(/\/$/, ""))
        .filter((value): value is string => Boolean(value)),
    ),
  );
}

export function register() {
  if (process.env.NEXT_RUNTIME === "edge") {
    return;
  }

  registerOTel({
    serviceName: process.env.OTEL_SERVICE_NAME || "bike-ui",
    instrumentationConfig: {
      fetch: {
        propagateContextUrls: tracePropagationUrls(),
      },
    },
  });
}
