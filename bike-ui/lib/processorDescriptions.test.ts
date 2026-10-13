// @vitest-environment node

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { processorDescription } from "./processorDescriptions";

describe("processor descriptions", () => {
  it("describes every processor type declared by the worker", () => {
    const source = readFileSync(
      new URL("../../bike-rs/bike-core/src/jobs/job_type.rs", import.meta.url),
      "utf8",
    );
    const types = new Set(
      Array.from(
        source.matchAll(/Self::\w+\s*=>\s*"([a-z_]+)"/g),
        (match) => match[1],
      ),
    );
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
