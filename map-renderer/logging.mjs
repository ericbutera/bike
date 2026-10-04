import { trace } from "@opentelemetry/api";
import { errorLogRecord } from "./error-log-record.mjs";

export { errorLogRecord } from "./error-log-record.mjs";

export function logError(message, error, fields = {}) {
  console.error(
    JSON.stringify(
      errorLogRecord(
        message,
        error,
        fields,
        trace.getActiveSpan()?.spanContext(),
      ),
    ),
  );
}
