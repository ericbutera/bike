import type { Attributes, Context, Link, SpanKind } from "@opentelemetry/api";
import { registerOTel } from "@vercel/otel";

const SAMPLING_DECISION_NOT_RECORD = 0;
const SAMPLING_DECISION_RECORD_AND_SAMPLED = 2;

const UI_REQUEST_SPAN_TYPE = "BaseServer.handleRequest";

type SamplingResult = {
  decision:
    | typeof SAMPLING_DECISION_NOT_RECORD
    | typeof SAMPLING_DECISION_RECORD_AND_SAMPLED;
  attributes?: Readonly<Attributes>;
};

function isUiRequestSpan(attributes: Attributes) {
  const spanType = attributes["next.span_type"];

  return spanType === UI_REQUEST_SPAN_TYPE;
}

const bikeUiTraceSampler = {
  shouldSample(
    _context: Context,
    _traceId: string,
    _spanName: string,
    _spanKind: SpanKind,
    attributes: Attributes,
    _links: Link[],
  ): SamplingResult {
    return {
      decision: isUiRequestSpan(attributes)
        ? SAMPLING_DECISION_RECORD_AND_SAMPLED
        : SAMPLING_DECISION_NOT_RECORD,
    };
  },
  toString() {
    return "BikeUiTraceSampler";
  },
};

export function register() {
  if (process.env.NEXT_RUNTIME === "edge") {
    return;
  }

  registerOTel({
    serviceName: process.env.OTEL_SERVICE_NAME || "bike-ui",
    instrumentations: [],
    traceSampler: bikeUiTraceSampler,
  });
}
