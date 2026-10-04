import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import ActivityRoutePreview from "../activity-stream/ActivityRoutePreview";

describe("ActivityRoutePreview", () => {
  beforeEach(() =>
    document.documentElement.setAttribute("data-theme", "light"),
  );

  it("updates the cached PNG URL when the applied theme changes", async () => {
    render(
      <ActivityRoutePreview
        activityId={7}
        title="Ride"
        showFullMap
        routePoints={[
          { elapsed_seconds: 0, latitude: 45, longitude: -85 },
          { elapsed_seconds: 60, latitude: 45.01, longitude: -85.01 },
        ]}
      />,
    );
    const image = screen.getByRole("img", { name: "Route map for Ride" });
    expect(image.getAttribute("src")).toContain("theme=light");
    act(() => document.documentElement.setAttribute("data-theme", "dark"));
    await waitFor(() =>
      expect(image.getAttribute("src")).toContain("theme=dark"),
    );
  });
});
