// @vitest-environment node

import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { processorDescription } from "./processorDescriptions";

describe("processor descriptions", () => {
  it("describes every processor type declared by the worker", () => {
    const directory = new URL(
      "../../bike-rs/worker/src/tasks/processors/",
      import.meta.url,
    );
    const types = new Set<string>();
    for (const file of readdirSync(directory).filter((file) =>
      file.endsWith(".rs"),
    )) {
      const source = readFileSync(new URL(file, directory), "utf8");
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
});
