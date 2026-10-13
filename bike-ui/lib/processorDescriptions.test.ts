// @vitest-environment node

import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { processorDescription } from "./processorDescriptions";

describe("processor descriptions", () => {
  it("describes every processor type declared by the worker", () => {
    const types = new Set<string>();
    const files = ["processors/", "strava/"].flatMap((path) => {
      const directory = new URL(
        `../../bike-rs/worker/src/tasks/${path}`,
        import.meta.url,
      );
      return readdirSync(directory)
        .filter((file) => file.endsWith(".rs"))
        .map((file) => new URL(file, directory));
    });
    files.push(
      new URL(
        "../../bike-rs/bike-core/src/strava/workflow/action.rs",
        import.meta.url,
      ),
    );
    for (const file of files) {
      const source = readFileSync(file, "utf8");
      const body = source.match(/fn task_type\([^]*?\n    }/)?.[0] ?? "";
      for (const match of body.matchAll(/"([a-z_]+)"/g)) types.add(match[1]);
    }
    expect(types.size).toBeGreaterThan(0);
    for (const type of types) {
      expect(processorDescription(type), type).not.toContain(
        "not available yet",
      );
    }
  });

  it("explains unknown processors without inventing their behavior", () => {
    for (const type of ["future_processor", "constructor", "__proto__"]) {
      expect(processorDescription(type)).toContain("not available yet");
    }
  });

  it.each([
    ["strava_activity_create", "created"],
    ["strava_activity_update", "changed"],
    ["strava_activity_upsert", "discovered"],
    ["strava_activity_delete", "deleted"],
    ["strava_sync", "after they complete"],
    ["strava_backfill", "historical"],
    ["strava_disconnect", "until cleanup completes"],
    ["strava_reconcile", "on a schedule"],
    ["strava_workflow", "older queued"],
  ])("explains the separate %s operation", (type, operation) => {
    expect(processorDescription(type)).toContain(operation);
  });
});
