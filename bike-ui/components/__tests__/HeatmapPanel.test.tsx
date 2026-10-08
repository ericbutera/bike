import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import HeatmapPanel from "../HeatmapPanel";
import { HEATMAP_COLOR_STORAGE_KEY } from "../../lib/heatmapColors";
import type { HeatmapView } from "../../lib/heatmapCamera";

const mocks = vi.hoisted(() => ({
  enabled: true,
  flagsPending: false,
  flagsError: false,
  retryFlags: vi.fn(),
  search: "",
  replace: vi.fn(),
  query: vi.fn(),
  refetch: vi.fn(),
  data: undefined as unknown,
  storage: new Map<string, string>(),
  view: undefined as HeatmapView | undefined,
}));
vi.mock("next/navigation", () => ({
  useRouter: () => ({ replace: mocks.replace }),
  useSearchParams: () => new URLSearchParams(mocks.search),
}));
vi.mock("next/dynamic", () => ({
  default: () => (props: { view: HeatmapView }) => {
    mocks.view = props.view;
    return <div aria-label="Personal activity heatmap" />;
  },
}));
vi.mock("../../lib/auth", () => ({
  useAuth: () => ({ user: { id: "viewer-1" } }),
}));
vi.mock("../../lib/featureFlags", () => ({
  FLAG_HEATMAPS: "heatmaps",
}));
vi.mock("../../lib/queries", () => ({
  usePublicFeatureFlags: () => ({
    data: [{ feature_key: "heatmaps", enabled: mocks.enabled }],
    isPending: mocks.flagsPending,
    isError: mocks.flagsError,
    refetch: mocks.retryFlags,
  }),
}));
vi.mock("../../lib/api", () => ({
  $typedApi: {
    queryOptions: () => ({ queryKey: ["get", "/maps/heatmap"] }),
    useQuery: (...args: unknown[]) => {
      mocks.query(...args);
      return {
        data: mocks.data,
        isFetching: false,
        isError: false,
        refetch: mocks.refetch,
      };
    },
  },
}));

beforeEach(() => {
  vi.clearAllMocks();
  mocks.enabled = true;
  mocks.flagsPending = false;
  mocks.flagsError = false;
  mocks.search = "";
  mocks.data = undefined;
  mocks.storage.clear();
  window.history.replaceState(null, "", "/maps");
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => mocks.storage.get(key) ?? null,
    setItem: (key: string, value: string) => mocks.storage.set(key, value),
  });
});
describe("personal heatmap", () => {
  it("shows availability loading without heatmap requests or a disabled message", () => {
    mocks.flagsPending = true;
    render(<HeatmapPanel />);
    expect(screen.getByRole("status")).toHaveTextContent(
      "Checking heatmap availability",
    );
    expect(mocks.query.mock.calls.at(-1)![3].enabled).toBe(false);
    expect(
      screen.queryByText("Heatmaps are not enabled on this site."),
    ).not.toBeInTheDocument();
  });
  it("retries an availability error without requesting heatmap data", () => {
    mocks.flagsError = true;
    render(<HeatmapPanel />);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Unable to check heatmap availability.",
    );
    expect(mocks.query.mock.calls.at(-1)![3].enabled).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(mocks.retryFlags).toHaveBeenCalledOnce();
  });
  it("makes no heatmap requests when the capability is disabled", () => {
    mocks.enabled = false;
    render(<HeatmapPanel />);
    expect(mocks.query.mock.calls.at(-1)![3].enabled).toBe(false);
    expect(
      screen.getByText("Heatmaps are not enabled on this site."),
    ).toBeVisible();
    expect(
      screen.queryByLabelText("Personal activity heatmap"),
    ).not.toBeInTheDocument();
  });
  it("rejects invalid URL filters before enabling a request", () => {
    mocks.search = "sport=unknown";
    render(<HeatmapPanel />);
    expect(mocks.query.mock.calls.at(-1)![3].enabled).toBe(false);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "supported activity type",
    );
  });
  it("remembers the selected color and restores it on the next visit", async () => {
    const { unmount } = render(<HeatmapPanel />);
    // Native disclosure hides the radios until the picker is opened.
    fireEvent.click(screen.getByLabelText("Heatmap color: Blue"));
    fireEvent.click(screen.getByLabelText("Orange"));
    expect(window.localStorage.getItem(HEATMAP_COLOR_STORAGE_KEY)).toBe(
      "orange",
    );
    const pathLegend = screen.getByLabelText("Activities per path legend");
    const selectedColor = pathLegend
      .querySelector("span[aria-hidden]")!
      .getAttribute("style");
    unmount();
    render(<HeatmapPanel />);
    await waitFor(() =>
      expect(
        screen.getByLabelText("Heatmap color: Orange"),
      ).toBeInTheDocument(),
    );
    const help = screen.getByLabelText("Heatmap help and legend");
    expect(help.closest("details")).not.toHaveAttribute("open");
    expect(help.closest("details")).toHaveClass("dropdown-end");
    const legend = screen.getByLabelText("Activities per path legend");
    expect(legend.closest("details")).not.toHaveAttribute("open");
    fireEvent.click(help);
    expect(legend.closest("details")).toHaveAttribute("open");
    expect(legend).toHaveTextContent("25+");
    expect(screen.queryByLabelText("Find my location")).not.toBeInTheDocument();
    expect(
      screen.queryByText(/Only your activities are shown/),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Fit routes")).not.toBeInTheDocument();
    expect(screen.queryByText("Refresh")).not.toBeInTheDocument();
    expect(
      screen
        .getByLabelText("Activities per path legend")
        .querySelector("span[aria-hidden]")!
        .getAttribute("style"),
    ).toBe(selectedColor);
  });
  it("shows partial preparation and shares selected filters in the URL", async () => {
    mocks.data = {
      revision: "4",
      filters: {},
      ready: 25,
      pending: 10,
      preparing: true,
      failed: 0,
      skipped: 2,
      bounds: [-85, 44, -84, 45],
      zones: [],
    };
    render(<HeatmapPanel />);
    await waitFor(() =>
      expect(mocks.query.mock.calls.at(-1)![3].enabled).toBe(true),
    );
    const zoneRequest = mocks.query.mock.calls
      .filter((call) => call[1] === "/maps/heatmap/zones")
      .at(-1);
    expect(zoneRequest?.[2]).toMatchObject({
      params: { query: { revision: "4" } },
    });
    expect(screen.getByRole("status")).toHaveTextContent(
      "Preparing 10 activities",
    );
    expect(
      screen.getByText(/25 activities with routes/).closest("details"),
    ).not.toHaveAttribute("open");
    expect(
      screen.getByLabelText("Activities per path legend"),
    ).toHaveTextContent("25+");
    fireEvent.change(screen.getByLabelText("Activity type"), {
      target: { value: "road_ride" },
    });
    expect(mocks.replace.mock.calls.at(-1)![0]).toContain("sport=road_ride");
    expect(mocks.replace.mock.calls.at(-1)![0]).toContain("tz=");
  });
  it("places zoom presets and help in the top-right toolbar without a title card", () => {
    render(<HeatmapPanel />);
    const toolbar = screen.getByRole("toolbar", { name: "Heatmap controls" });
    const summaries = toolbar.querySelectorAll("summary");
    expect(
      [...summaries].map(
        (summary) => summary.getAttribute("aria-label") ?? summary.textContent,
      ),
    ).toEqual([
      "Zoom preset",
      "Heatmap color: Blue",
      "Filters",
      "Heatmap help and legend",
    ]);
    expect(
      screen.queryByRole("heading", { name: "Your heatmap" }),
    ).not.toBeInTheDocument();
    expect(toolbar.querySelector('button[aria-label*="location"]')).toBeNull();
    fireEvent.click(screen.getByLabelText("Heatmap help and legend"));
    expect(screen.getByText(/Paths become more opaque/)).toBeVisible();
    expect(screen.getByLabelText("Activities per path legend")).toBeVisible();
  });
  it("requests Region and Full without changing URL filters or color", () => {
    mocks.data = {
      ready: 1,
      bounds: [-85, 44, -84, 45],
      filters: {},
      revision: "fixture",
      style_version: "heatmap-v2",
      zones: [],
    };
    mocks.search = "sport=road_ride&start=2026-01-01";
    const { rerender } = render(<HeatmapPanel />);
    fireEvent.click(screen.getByText("Zoom preset", { exact: true }));
    fireEvent.click(screen.getByRole("button", { name: "Region" }));
    expect(mocks.view?.request).toEqual({ action: "region", sequence: 1 });
    fireEvent.click(screen.getByText("Zoom preset", { exact: true }));
    fireEvent.click(screen.getByRole("button", { name: "Full" }));
    expect(mocks.view?.request).toEqual({ action: "full", sequence: 2 });
    expect(mocks.replace).not.toHaveBeenCalled();
    expect(mocks.storage.size).toBe(0);
    mocks.data = { ready: 0, bounds: null };
    rerender(<HeatmapPanel />);
    expect(
      screen.getByRole("button", { name: "Full", hidden: true }),
    ).toBeDisabled();
  });
  it("restores a saved camera and saves movement without removing filters", () => {
    mocks.search = "sport=road_ride&lng=-122&lat=47&zoom=9";
    window.history.replaceState(null, "", `/maps?${mocks.search}`);
    render(<HeatmapPanel />);
    expect(mocks.view?.camera).toEqual({
      longitude: -122,
      latitude: 47,
      zoom: 9,
    });
    mocks.view!.onChange({ longitude: -85, latitude: 45, zoom: 13 });
    const params = new URLSearchParams(window.location.search);
    expect(params.get("sport")).toBe("road_ride");
    expect(params.get("lng")).toBe("-85");
    expect(params.get("zoom")).toBe("13");
  });
});
