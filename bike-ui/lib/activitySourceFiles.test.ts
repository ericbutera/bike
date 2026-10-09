import { describe, expect, it, vi } from "vitest";
import { activitySourceFileUrl } from "./activitySourceFiles";

vi.mock("./config", () => ({ config: { API_URL: "/api/" } }));

describe("activity source files", () => {
  it("builds the original recording download URL for an activity", () => {
    expect(activitySourceFileUrl(42)).toBe("/api/activities/42/source-file");
  });

  it("keeps a string activity identifier inside one URL segment", () => {
    expect(activitySourceFileUrl("ride/42")).toBe(
      "/api/activities/ride%2F42/source-file",
    );
  });
});
