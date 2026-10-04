import { describe, expect, it } from "vitest";
import {
  calendarDayStart,
  heatmapPreset,
  heatmapTileUrl,
  parseHeatmapFilters,
} from "./heatmaps";

describe("heatmap calendar filters", () => {
  it("changes tile URLs when the renderer style version changes", () => {
    const metadata = {
      revision: "1",
      filters: {},
      ready: 1,
      pending: 0,
      failed: 0,
      skipped: 0,
      bounds: null,
      preparing: false,
      min_zoom: 0,
      max_zoom: 18,
      tile_size: 512,
      style_version: "heatmap-v1",
    };
    const oldUrl = heatmapTileUrl(metadata);
    expect(
      heatmapTileUrl({ ...metadata, style_version: "heatmap-v2" }),
    ).not.toBe(oldUrl);
    expect(oldUrl).toContain("/{z}/{x}/{y}.png");
  });
  it.each([
    ["2026-03-08", 23],
    ["2026-11-01", 25],
  ])("uses the full %s local day across DST", (day, hours) => {
    const filters = parseHeatmapFilters(
      new URLSearchParams({
        start: day,
        end: day,
        tz: "America/Detroit",
        sport: "road_ride",
      }),
      "UTC",
    );
    expect(filters.error).toBeUndefined();
    expect(
      (Date.parse(filters.query.to!) - Date.parse(filters.query.from!)) /
        3_600_000,
    ).toBe(hours);
    expect(filters.query.sport).toBe("road_ride");
  });
  it("rejects malformed, reversed and unknown URL filters", () => {
    const invalidFilters: Record<string, string>[] = [
      { sport: "flying" },
      { tz: "not/a-zone" },
      { start: "2026-02-30" },
      { start: "2026-10-03", end: "2026-10-01" },
    ];
    for (const values of invalidFilters) {
      expect(
        parseHeatmapFilters(new URLSearchParams(values), "UTC").error,
      ).toBeTruthy();
    }
  });
  it("supports midnight timezone transitions and calendar presets", () => {
    expect(calendarDayStart("2018-11-04", "America/Sao_Paulo")).toBe(
      "2018-11-04T03:00:00.000Z",
    );
    expect(
      heatmapPreset(
        "month",
        "America/Detroit",
        new Date("2026-03-10T12:00:00Z"),
      ),
    ).toEqual({ start: "2026-02-09", end: "2026-03-10" });
    expect(heatmapPreset("all", "UTC")).toEqual({ start: "", end: "" });
  });
});
