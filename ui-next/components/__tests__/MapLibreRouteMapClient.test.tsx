import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import MapLibreRouteMapClient from "../MapLibreRouteMapClient";

const mapMocks = vi.hoisted(() => ({
  fitBounds: vi.fn(),
  jumpTo: vi.fn(),
  setStyle: vi.fn(),
  addControl: vi.fn(),
  createMap: vi.fn(),
  sources: new Map<string, { setData: ReturnType<typeof vi.fn> }>(),
}));

vi.mock("maplibre-gl", () => {
  class MockLngLatBounds {
    points: [number, number][];

    constructor(sw: [number, number], ne: [number, number]) {
      this.points = [sw, ne];
    }

    extend(point: [number, number]) {
      this.points.push(point);
      return this;
    }
  }

  class MockMap {
    constructor() {
      const layers = new globalThis.Map<
        string,
        { id: string; type?: string }
      >();
      const canvas = { style: { cursor: "" } };

      const map = {
        addControl: mapMocks.addControl,
        addSource: vi.fn((id: string) => {
          mapMocks.sources.set(id, { setData: vi.fn() });
        }),
        addLayer: vi.fn((layer: { id: string; type?: string }) => {
          layers.set(layer.id, layer);
        }),
        getSource: vi.fn((id: string) => mapMocks.sources.get(id)),
        getLayer: vi.fn((id: string) => layers.get(id)),
        getStyle: vi.fn(() => ({
          layers: [{ id: "road-label", type: "symbol" }],
        })),
        isStyleLoaded: vi.fn(() => true),
        once: vi.fn(),
        off: vi.fn(),
        on: vi.fn(),
        getCanvas: vi.fn(() => canvas),
        fitBounds: mapMocks.fitBounds,
        getCenter: vi.fn(() => ({ lng: -122, lat: 45 })),
        getZoom: vi.fn(() => 13),
        getBearing: vi.fn(() => 0),
        getPitch: vi.fn(() => 0),
        jumpTo: mapMocks.jumpTo,
        setStyle: mapMocks.setStyle,
        resize: vi.fn(),
        remove: vi.fn(),
      };

      mapMocks.createMap(map);

      return map;
    }
  }

  class MockNavigationControl {}
  class MockAttributionControl {}

  return {
    default: {
      Map: MockMap,
      NavigationControl: MockNavigationControl,
      AttributionControl: MockAttributionControl,
      LngLatBounds: MockLngLatBounds,
    },
  };
});

describe("MapLibreRouteMapClient", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mapMocks.sources.clear();
  });

  it("does not refit bounds when only playback markers change", async () => {
    const routePoints = [
      { elapsed_seconds: 0, latitude: 45.0, longitude: -122.0 },
      { elapsed_seconds: 120, latitude: 45.004, longitude: -121.996 },
      { elapsed_seconds: 240, latitude: 45.008, longitude: -121.992 },
    ];

    const { rerender } = render(
      <MapLibreRouteMapClient
        routePoints={routePoints}
        movingMarkers={[
          {
            id: "1",
            point: routePoints[0],
            color: "#0f766e",
          },
        ]}
        ariaLabel="Segment comparison map"
        emptyMessage="Segment route geometry is not available yet."
        fitBoundsPadding={24}
        fitBoundsMaxZoom={18}
      />,
    );

    await waitFor(() => {
      expect(mapMocks.fitBounds).toHaveBeenCalledTimes(1);
    });

    expect(mapMocks.fitBounds).toHaveBeenLastCalledWith(
      expect.any(Object),
      expect.objectContaining({
        padding: 24,
        maxZoom: 18,
        duration: 0,
      }),
    );

    rerender(
      <MapLibreRouteMapClient
        routePoints={routePoints}
        movingMarkers={[
          {
            id: "1",
            point: routePoints[1],
            color: "#0f766e",
          },
        ]}
        ariaLabel="Segment comparison map"
        emptyMessage="Segment route geometry is not available yet."
        fitBoundsPadding={24}
        fitBoundsMaxZoom={18}
      />,
    );

    await waitFor(() => {
      expect(
        mapMocks.sources.get("activity-route-markers")?.setData,
      ).toHaveBeenCalledTimes(2);
    });

    expect(mapMocks.fitBounds).toHaveBeenCalledTimes(1);
    expect(mapMocks.addControl).toHaveBeenCalledTimes(1);
  });
});
