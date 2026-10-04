import assert from "node:assert/strict";
import { test } from "node:test";
import { errorLogRecord } from "./error-log-record.mjs";

test("structured error logs correlate to the active trace without logging raw errors", () => {
  const record = errorLogRecord(
    "Map render failed",
    new Error("request included a sensitive value"),
    { stage: "browser_render" },
    {
      traceId: "11111111111111111111111111111111",
      spanId: "2222222222222222",
    },
  );

  assert.deepEqual(record, {
    level: "error",
    message: "Map render failed",
    stage: "browser_render",
    error_type: "Error",
    trace_id: "11111111111111111111111111111111",
    span_id: "2222222222222222",
  });
  assert.equal(JSON.stringify(record).includes("sensitive value"), false);
});

test("error records keep stage fields as the third argument", () => {
  const record = errorLogRecord(
    "Map render failed",
    new Error("private input"),
    { stage: "http_render" },
    {
      traceId: "33333333333333333333333333333333",
      spanId: "4444444444444444",
    },
  );

  assert.deepEqual(record, {
    level: "error",
    message: "Map render failed",
    stage: "http_render",
    error_type: "Error",
    trace_id: "33333333333333333333333333333333",
    span_id: "4444444444444444",
  });
});
