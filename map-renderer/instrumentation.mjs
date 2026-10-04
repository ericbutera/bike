import { NodeSDK } from "@opentelemetry/sdk-node";
import { OTLPTraceExporter } from "@opentelemetry/exporter-trace-otlp-http";
import { resourceFromAttributes } from "@opentelemetry/resources";
import { AlwaysOnSampler } from "@opentelemetry/sdk-trace-base";
import { HttpInstrumentation } from "@opentelemetry/instrumentation-http";
import { GrpcInstrumentation } from "@opentelemetry/instrumentation-grpc";

const exporterNames = (process.env.OTEL_TRACES_EXPORTER ?? "otlp")
  .split(",")
  .map((value) => value.trim().toLowerCase());
const exporterEnabled =
  process.env.OTEL_SDK_DISABLED?.toLowerCase() !== "true" &&
  exporterNames.includes("otlp");

export const sdk = new NodeSDK({
  resource: resourceFromAttributes({
    "service.name": process.env.OTEL_SERVICE_NAME ?? "bike-map-renderer",
    "service.version": process.env.BIKE_VERSION ?? "unknown",
    "deployment.environment.name": process.env.APP_ENV ?? "development",
  }),
  sampler: new AlwaysOnSampler(),
  ...(exporterEnabled ? { traceExporter: new OTLPTraceExporter() } : {}),
  instrumentations: [
    new HttpInstrumentation({
      ignoreIncomingRequestHook: (request) => {
        const path = request.url?.split("?", 1)[0] ?? "";
        return (
          path === "/healthz" ||
          path === "/metrics" ||
          path === "/" ||
          path.startsWith("/vendor/") ||
          path.startsWith("/styles/")
        );
      },
    }),
    new GrpcInstrumentation(),
  ],
});

await sdk.start();

export async function shutdownTracing() {
  await sdk.shutdown();
}
