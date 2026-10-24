import { readFile } from "node:fs/promises";
import { gunzipSync } from "node:zlib";
import { connectedTest as test, expect } from "./helpers/test.mjs";
import { environmentCommand } from "./helpers/environment.mjs";

test("activity 1671 runs every shared DAG stage and an unchanged retry in PostgreSQL", async () => {
  const bytes = await readFile(
    new URL(
      "../../../bike-rs/tests/fixtures/activity_dag/activity-1671.json.gz",
      import.meta.url,
    ),
  );
  const result = await environmentCommand(
    "seed",
    gunzipSync(bytes).toString("utf8"),
  );
  const report = JSON.parse(result.stdout.split("\n")[0]);
  await test.info().attach("activity-dag", {
    body: JSON.stringify(report, null, 2),
    contentType: "application/json",
  });
  expect(report.failures).toEqual([]);
  expect(report.workflow_ms).toHaveLength(2);
  expect(report.passed).toContain(
    "unchanged retry preserves effort IDs, awards and recording reference",
  );
});
