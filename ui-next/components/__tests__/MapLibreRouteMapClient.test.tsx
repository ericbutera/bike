import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import MapLibreRouteMapClient from "../MapLibreRouteMapClient";

const mapMocks = vi.hoisted(() => ({
  easeTo: vi.fn(),
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
        easeTo: mapMocks.easeTo,
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

  it("jumps to follow mode initially, then smooths leader tracking without refitting bounds", async () => {
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
        followViewport={{ point: routePoints[0], zoom: 19 }}
        ariaLabel="Segment comparison map"
        emptyMessage="Segment route geometry is not available yet."
        fitBoundsPadding={24}
        fitBoundsMaxZoom={18}
      />,
    );

    await waitFor(() => {
      expect(mapMocks.jumpTo).toHaveBeenCalledWith(
        expect.objectContaining({
          center: [-122.0, 45.0],
          zoom: 19,
        }),
      );
    });

    expect(mapMocks.fitBounds).toHaveBeenCalledTimes(1);
    expect(mapMocks.easeTo).not.toHaveBeenCalled();

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
        followViewport={{ point: routePoints[1], zoom: 17 }}
        ariaLabel="Segment comparison map"
        emptyMessage="Segment route geometry is not available yet."
        fitBoundsPadding={24}
        fitBoundsMaxZoom={18}
      />,
    );

    await waitFor(() => {
      expect(mapMocks.easeTo).toHaveBeenLastCalledWith(
        expect.objectContaining({
          center: [-121.99936, 45.00064],
          zoom: 18.92,
          duration: 1400,
          easing: expect.any(Function),
        }),
      );
    });

    expect(mapMocks.fitBounds).toHaveBeenCalledTimes(1);
  });

  it("refits bounds when a focused point set changes", async () => {
    const routePoints = [
      { elapsed_seconds: 0, latitude: 45.0, longitude: -122.0 },
      { elapsed_seconds: 120, latitude: 45.004, longitude: -121.996 },
      { elapsed_seconds: 240, latitude: 45.008, longitude: -121.992 },
    ];
    const climbPoints = routePoints.slice(1);

    const { rerender } = render(
      <MapLibreRouteMapClient
        routePoints={routePoints}
        ariaLabel="Activity route map"
        emptyMessage="Activity route geometry is not available yet."
        fitBoundsKey="activity"
        fitBoundsPadding={24}
        fitBoundsMaxZoom={18}
      />,
    );

    await waitFor(() => {
      expect(mapMocks.fitBounds).toHaveBeenCalledTimes(1);
    });

    rerender(
      <MapLibreRouteMapClient
        routePoints={routePoints}
        fitBoundsPoints={climbPoints}
        fitBoundsKey="climb-1"
        ariaLabel="Activity route map"
        emptyMessage="Activity route geometry is not available yet."
        fitBoundsPadding={24}
        fitBoundsMaxZoom={18}
      />,
    );

    await waitFor(() => {
      expect(mapMocks.fitBounds).toHaveBeenCalledTimes(2);
    });

    const focusedBounds = mapMocks.fitBounds.mock.calls.at(-1)?.[0] as
      | { points: [number, number][] }
      | undefined;

    expect(focusedBounds?.points).toContainEqual([-121.996, 45.004]);
    expect(focusedBounds?.points).toContainEqual([-121.992, 45.008]);
    expect(focusedBounds?.points).not.toContainEqual([-122.0, 45.0]);
  });

  it("animates markers along route progress instead of cutting across the path", async () => {
    const animationCallbacks: FrameRequestCallback[] = [];
    const performanceNowSpy = vi.spyOn(performance, "now");
    let nowMs = 0;

    performanceNowSpy.mockImplementation(() => nowMs);
    vi.stubGlobal("requestAnimationFrame", ((
      callback: FrameRequestCallback,
    ) => {
      animationCallbacks.push(callback);
      return animationCallbacks.length;
    }) as typeof requestAnimationFrame);
    vi.stubGlobal(
      "cancelAnimationFrame",
      vi.fn() as typeof cancelAnimationFrame,
    );

    try {
      const routePoints = [
        {
          elapsed_seconds: 0,
          latitude: 0,
          longitude: 0,
          distance_meters: 0,
        },
        {
          elapsed_seconds: 60,
          latitude: 1,
          longitude: 0,
          distance_meters: 100,
        },
        {
          elapsed_seconds: 120,
          latitude: 1,
          longitude: 1,
          distance_meters: 200,
        },
      ];

      const { rerender } = render(
        <MapLibreRouteMapClient
          routePoints={routePoints}
          movingMarkers={[
            {
              id: "1",
              point: {
                elapsed_seconds: 30,
                latitude: 0.5,
                longitude: 0,
                distance_meters: 50,
              },
              progress: 0.25,
              color: "#0f766e",
            },
          ]}
          movingMarkerTransitionMs={160}
          ariaLabel="Segment comparison map"
          emptyMessage="Segment route geometry is not available yet."
          fitBoundsPadding={24}
          fitBoundsMaxZoom={18}
        />,
      );

      await waitFor(() => {
        expect(
          mapMocks.sources.get("activity-route-markers")?.setData,
        ).toHaveBeenCalledTimes(1);
      });

      animationCallbacks.length = 0;

      nowMs = 100;

      rerender(
        <MapLibreRouteMapClient
          routePoints={routePoints}
          movingMarkers={[
            {
              id: "1",
              point: {
                elapsed_seconds: 90,
                latitude: 1,
                longitude: 0.5,
                distance_meters: 150,
              },
              progress: 0.75,
              color: "#0f766e",
            },
          ]}
          movingMarkerTransitionMs={160}
          ariaLabel="Segment comparison map"
          emptyMessage="Segment route geometry is not available yet."
          fitBoundsPadding={24}
          fitBoundsMaxZoom={18}
        />,
      );

      expect(animationCallbacks.length).toBe(1);

      const nextAnimation = animationCallbacks[0];

      expect(nextAnimation).toBeDefined();

      nextAnimation?.(150);

      const markerSetDataCalls = mapMocks.sources.get("activity-route-markers")
        ?.setData.mock.calls;
      const intermediateSourceData = markerSetDataCalls?.at(-1)?.[0];
      const coordinates = intermediateSourceData?.features?.[0]?.geometry
        ?.coordinates as [number, number];

      expect(coordinates[0]).toBeCloseTo(0, 5);
      expect(coordinates[1]).toBeCloseTo(1, 5);
    } finally {
      performanceNowSpy.mockRestore();
      vi.unstubAllGlobals();
    }
  });
});
