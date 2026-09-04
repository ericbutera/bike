import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import SegmentEffortAnalysisSection from "../segment-detail/SegmentEffortAnalysisSection";

const mocks = vi.hoisted(() => ({
  useSegmentEffortAnalysis: vi.fn(),
  renderMapLibreRouteMap: vi.fn(),
}));

vi.mock("../../lib/queries", () => ({
  useSegmentEffortAnalysis: mocks.useSegmentEffortAnalysis,
}));

vi.mock("../MapLibreRouteMap", () => ({
  default: (props: any) => {
    mocks.renderMapLibreRouteMap(props);

    return <div role="img" aria-label={props.ariaLabel} />;
  },
}));

vi.mock("next/link", () => ({
  default: ({ href, children, ...props }: any) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

vi.mock("recharts", () => ({
  Bar: () => null,
  BarChart: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  CartesianGrid: () => null,
  Cell: () => null,
  Legend: () => null,
  ResponsiveContainer: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  Tooltip: () => null,
  XAxis: () => null,
  YAxis: () => null,
}));

function makeRoutePoint(
  elapsed_seconds: number,
  latitude: number,
  longitude: number,
) {
  return {
    elapsed_seconds,
    latitude,
    longitude,
    distance_meters: elapsed_seconds * 10,
    elevation_meters: 120 + elapsed_seconds,
    speed_mps: 8,
    heart_rate_bpm: 140,
  };
}

describe("SegmentEffortAnalysisSection", () => {
  it("fits the section map tightly enough for short segment geometry", () => {
    const routePoints = [
      makeRoutePoint(0, 45.0, -122.0),
      makeRoutePoint(60, 45.001, -121.999),
      makeRoutePoint(120, 45.002, -121.998),
    ];

    mocks.useSegmentEffortAnalysis.mockReturnValue({
      data: {
        segment_id: 14,
        segment_title: "North Climb",
        split_count: 10,
        route_points: routePoints,
        reference_effort: {
          effort_id: 1,
          activity_id: 7,
          activity_title: "Lunch Ride",
          activity_started_at: "2026-05-06T12:00:00Z",
          effort_index: 1,
          duration_seconds: 120,
          delta_from_reference_seconds: 0,
        },
        efforts: [
          {
            effort_id: 1,
            activity_id: 7,
            activity_title: "Lunch Ride",
            activity_started_at: "2026-05-06T12:00:00Z",
            effort_index: 1,
            duration_seconds: 120,
            delta_from_reference_seconds: 0,
          },
        ],
        sections: [
          {
            section_index: 1,
            start_progress_percent: 0,
            end_progress_percent: 100,
            reference_split_seconds: 120,
            best_split_seconds: 118,
            best_effort_id: 1,
            best_activity_id: 7,
            best_activity_title: "Lunch Ride",
            gain_available_seconds: 2,
            top_efforts: [
              {
                effort_id: 1,
                activity_id: 7,
                activity_title: "Lunch Ride",
                activity_started_at: "2026-05-06T12:00:00Z",
                split_seconds: 120,
                delta_from_reference_seconds: 0,
                delta_from_best_seconds: 2,
              },
            ],
            efforts: [
              {
                effort_id: 1,
                activity_id: 7,
                activity_title: "Lunch Ride",
                activity_started_at: "2026-05-06T12:00:00Z",
                split_seconds: 120,
                delta_from_reference_seconds: 0,
                delta_from_best_seconds: 2,
              },
            ],
          },
        ],
        theoretical_best_duration_seconds: 118,
        theoretical_best_gain_seconds: 2,
      },
      isLoading: false,
      isError: false,
      error: null,
    });

    render(<SegmentEffortAnalysisSection segment={{ id: 14 } as any} />);

    expect(
      screen.getByRole("img", { name: "North Climb analysis sections map" }),
    ).toBeInTheDocument();

    const mapProps = mocks.renderMapLibreRouteMap.mock.lastCall?.[0];

    expect(mapProps.fitBoundsPoints).toBe(routePoints);
    expect(mapProps.fitBoundsPadding).toBe(24);
    expect(mapProps.fitBoundsMaxZoom).toBe(18);
  });
});
