import { describe, expect, it } from "vitest";
import { buildInitialRouteState, buildSyncedTarget } from "../AuthRouter";

describe("AuthRouter helpers", () => {
  it("copies email from route state into the synced URL", () => {
    expect(
      buildSyncedTarget("/confirm-email", "", { email: "rider@example.com" }),
    ).toBe("/confirm-email?email=rider%40example.com");
  });

  it("preserves an existing email query string", () => {
    expect(
      buildSyncedTarget("/confirm-email", "?email=already%40example.com", {
        email: "new@example.com",
      }),
    ).toBe("/confirm-email?email=already%40example.com");
  });

  it("reconstructs route state from the email query parameter", () => {
    expect(
      buildInitialRouteState("/confirm-email", "rider@example.com"),
    ).toEqual({
      email: "rider@example.com",
    });
  });

  it("provides an empty email state on the confirm-email page", () => {
    expect(buildInitialRouteState("/confirm-email", null)).toEqual({
      email: "",
    });
  });

  it("does not synthesize state for unrelated routes", () => {
    expect(buildInitialRouteState("/login", null)).toBeNull();
  });
});
