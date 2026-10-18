import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LayerSpecification } from "maplibre-gl";
import MapLibreRouteMapClient from "../MapLibreRouteMapClient";

const mapMocks = vi.hoisted(() => ({
  easeTo: vi.fn(),
  fitBounds: vi.fn(),
  jumpTo: vi.fn(),
  setStyle: vi.fn(),
  addControl: vi.fn(),
  createMap: vi.fn(),
  sources: new Map<string, { setData: ReturnType<typeof vi.fn> }>(),
  sourceOptions: new Map<string, any>(),
  images: new Map<string, any>(),
  layers: new Map<string, LayerSpecification>(),
  handlers: new Map<string, Array<(event?: any) => void>>(),
  markers: [] as Array<{
    element: HTMLElement;
    anchor: string;
    setLngLat: ReturnType<typeof vi.fn>;
    remove: ReturnType<typeof vi.fn>;
  }>,
  zoom: 13,
  styleLoaded: true,
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
    constructor(options: { container: HTMLElement }) {
      const canvas = { style: { cursor: "" } };

      const map = {
        addControl: mapMocks.addControl,
        addSource: vi.fn((id: string, options: unknown) => {
          mapMocks.sources.set(id, { setData: vi.fn() });
          mapMocks.sourceOptions.set(id, options);
        }),
        hasImage: vi.fn((id: string) => mapMocks.images.has(id)),
        addImage: vi.fn((id: string, data: unknown) =>
          mapMocks.images.set(id, data),
        ),
        addLayer: vi.fn((layer: LayerSpecification) => {
          mapMocks.layers.set(layer.id, layer);
        }),
        getSource: vi.fn((id: string) => mapMocks.sources.get(id)),
        getLayer: vi.fn((id: string) => mapMocks.layers.get(id)),
        getStyle: vi.fn(() => ({
          layers: [{ id: "road-label", type: "symbol" }],
        })),
        isStyleLoaded: vi.fn(() => mapMocks.styleLoaded),
        once: vi.fn(),
        off: vi.fn(
          (
            eventName: string,
            layerOrHandler: string | ((event?: any) => void),
            maybeHandler?: (event?: any) => void,
          ) => {
            const handler =
              typeof layerOrHandler === "function"
                ? layerOrHandler
                : maybeHandler;
            const handlers = mapMocks.handlers.get(eventName) ?? [];
            mapMocks.handlers.set(
              eventName,
              handlers.filter((entry) => entry !== handler),
            );
          },
        ),
        on: vi.fn(
          (
            eventName: string,
            layerOrHandler: string | ((event?: any) => void),
            maybeHandler?: (event?: any) => void,
          ) => {
            const handler =
              typeof layerOrHandler === "function"
                ? layerOrHandler
                : maybeHandler;

            if (!handler) {
              return;
            }

            const handlers = mapMocks.handlers.get(eventName) ?? [];
            handlers.push(handler);
            mapMocks.handlers.set(eventName, handlers);
          },
        ),
        getCanvas: vi.fn(() => canvas),
        getContainer: vi.fn(() => options.container),
        easeTo: mapMocks.easeTo,
        fitBounds: mapMocks.fitBounds,
        getCenter: vi.fn(() => ({ lng: -122, lat: 45 })),
        getZoom: vi.fn(() => mapMocks.zoom),
        getBearing: vi.fn(() => 0),
        getPitch: vi.fn(() => 0),
        jumpTo: mapMocks.jumpTo,
        setStyle: mapMocks.setStyle,
        resize: vi.fn(),
        remove: vi.fn(),
      };

      mapMocks.createMap(map, options);

      return map;
    }
  }

  class MockNavigationControl {}
  class MockAttributionControl {}
  class MockMarker {
    element: HTMLElement;
    anchor: string;
    setLngLat = vi.fn(() => this);
    addTo = vi.fn((map: { getContainer: () => HTMLElement }) => {
      map.getContainer().append(this.element);
      return this;
    });
    remove = vi.fn(() => this.element.remove());

    constructor({ element, anchor }: { element: HTMLElement; anchor: string }) {
      this.element = element;
      this.anchor = anchor;
      mapMocks.markers.push(this);
    }
  }

  return {
    setWorkerUrl: vi.fn(),
    Map: MockMap,
    NavigationControl: MockNavigationControl,
    AttributionControl: MockAttributionControl,
    Marker: MockMarker,
    LngLatBounds: MockLngLatBounds,
  };
});

describe("MapLibreRouteMapClient", () => {
  beforeEach(() => {
    document.documentElement.setAttribute("data-theme", "light");
    vi.clearAllMocks();
    mapMocks.sources.clear();
    mapMocks.sourceOptions.clear();
    mapMocks.images.clear();
    mapMocks.layers.clear();
    mapMocks.handlers.clear();
    mapMocks.markers.length = 0;
    mapMocks.zoom = 13;
    mapMocks.styleLoaded = true;
  });

  it("uses the shared backgrounds and blue route across themes, preserving overlays and camera", async () => {
    const routePoints = [
      { elapsed_seconds: 0, latitude: 45, longitude: -85 },
      { elapsed_seconds: 60, latitude: 45.01, longitude: -85.01 },
    ];
    render(
      <MapLibreRouteMapClient
        routePoints={routePoints}
        overlays={[
          {
            id: "segment",
            points: routePoints,
            color: "#e11d48",
            weight: 6,
            casingColor: "#ffffff",
            casingWeight: 9,
          },
          { id: "route-highlight", points: routePoints },
        ]}
        ariaLabel="Activity route map"
        emptyMessage="No route"
      />,
    );

    await waitFor(() => {
      expect(mapMocks.sources.has("activity-route")).toBe(true);
    });
    expect(mapMocks.sources.has("cycling-trails")).toBe(false);
    expect(mapMocks.createMap).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({ style: "/map-styles/route-light-v1.json" }),
    );
    expect(mapMocks.layers.get("activity-route-line")?.paint).toEqual(
      expect.objectContaining({
        "line-color": ["get", "color"],
        "line-width": 6,
      }),
    );
    expect(
      mapMocks.sources.get("activity-route")?.setData.mock.lastCall?.[0]
        .features[0].properties.color,
    ).toBe("#0060df");
    expect(mapMocks.layers.get("activity-route-casing")?.paint).toEqual(
      expect.objectContaining({ "line-color": "#ffffff", "line-width": 9 }),
    );
    expect(
      mapMocks.layers.get("activity-route-overlays-casing")?.paint,
    ).toEqual(
      expect.objectContaining({
        "line-color": ["get", "casingColor"],
        "line-width": ["get", "casingWeight"],
      }),
    );

    await act(async () => {
      document.documentElement.setAttribute("data-theme", "dark");
    });
    await act(async () => {
      mapMocks.sources.clear();
      mapMocks.layers.clear();
      for (const handler of mapMocks.handlers.get("style.load") ?? []) {
        handler();
      }
    });
    await waitFor(() => {
      expect(mapMocks.setStyle).toHaveBeenCalledWith(
        "/map-styles/fiord-v1.json",
      );
      expect(mapMocks.sources.has("activity-route")).toBe(true);
      expect(mapMocks.sources.has("activity-route-overlays")).toBe(true);
      expect(mapMocks.markers).toHaveLength(2);
      expect(screen.getByLabelText("Route start")).toBeInTheDocument();
      expect(screen.getByLabelText("Route finish")).toBeInTheDocument();
      expect(mapMocks.sources.has("cycling-trails")).toBe(false);
      expect(mapMocks.jumpTo).toHaveBeenCalled();
      expect(mapMocks.layers.get("activity-route-line")?.paint).toEqual(
        expect.objectContaining({ "line-color": ["get", "color"] }),
      );
      expect(
        mapMocks.sources.get("activity-route")?.setData.mock.lastCall?.[0]
          .features[0].properties.color,
      ).toBe("#0060df");
      const overlays = mapMocks.sources.get("activity-route-overlays")?.setData
        .mock.lastCall?.[0];
      expect(overlays.features[0].properties.color).toBe("#e11d48");
      expect(overlays.features[0].properties).toMatchObject({
        weight: 6,
        casingColor: "#ffffff",
        casingWeight: 9,
      });
      expect(overlays.features[1].properties.color).toBe("#0060df");
      expect(overlays.features[1].properties).toMatchObject({
        weight: 6,
        casingColor: "#202b37",
        casingWeight: 10,
      });
    });
  });

  it("marks route endpoints with only a green play icon and a checkered flag", () => {
    const routePoints = [
      { elapsed_seconds: 0, latitude: 45, longitude: -85 },
      { elapsed_seconds: 60, latitude: 45.01, longitude: -85.01 },
    ];
    render(
      <MapLibreRouteMapClient
        routePoints={routePoints}
        ariaLabel="Activity route map"
        emptyMessage="No route"
      />,
    );
    expect(mapMocks.markers).toHaveLength(2);
    const [start, finish] = mapMocks.markers;
    for (const marker of [start, finish]) {
      expect(marker.anchor).toBe("center");
      expect(marker.element.querySelector("svg")).toHaveStyle({
        display: "block",
        width: "16px",
        height: "16px",
      });
    }
    expect(start.setLngLat).toHaveBeenCalledWith([-85, 45]);
    expect(finish.setLngLat).toHaveBeenCalledWith([-85.01, 45.01]);
    expect(start.element.textContent).toBe("");
    expect(start.element.querySelector('svg[data-icon="play"]')).toHaveStyle({
      color: "#16a34a",
    });
    expect(finish.element.textContent).toBe("");
    expect(
      finish.element.querySelector('svg[data-icon="flag-checkered"]'),
    ).toBeInTheDocument();
  });

  it("retains both endpoint icons for a closed route", () => {
    const point = { elapsed_seconds: 0, latitude: 45, longitude: -85 };
    render(
      <MapLibreRouteMapClient
        routePoints={[point, { ...point, elapsed_seconds: 60 }]}
        ariaLabel="Loop map"
        emptyMessage="No route"
      />,
    );
    expect(mapMocks.markers).toHaveLength(2);
    mapMocks.markers.forEach((marker) =>
      expect(marker.setLngLat).toHaveBeenCalledWith([-85, 45]),
    );
    expect(screen.getByLabelText("Route start")).toBeInTheDocument();
    expect(screen.getByLabelText("Route finish")).toBeInTheDocument();
  });

  it("updates endpoint positions and removes markers when hidden, missing geometry, or unmounted", () => {
    const routePoints = [
      { elapsed_seconds: 0, latitude: 45, longitude: -85 },
      { elapsed_seconds: 60, latitude: 45.01, longitude: -85.01 },
    ];
    const props = {
      routePoints,
      ariaLabel: "Activity route map",
      emptyMessage: "No route",
    };
    const { rerender, unmount } = render(<MapLibreRouteMapClient {...props} />);
    const initial = [...mapMocks.markers];
    rerender(
      <MapLibreRouteMapClient
        {...props}
        routePoints={[...routePoints].reverse()}
      />,
    );
    expect(mapMocks.markers).toEqual(initial);
    initial.forEach((marker) => expect(marker.remove).not.toHaveBeenCalled());
    expect(initial[0].setLngLat).toHaveBeenLastCalledWith([-85.01, 45.01]);
    expect(initial[1].setLngLat).toHaveBeenLastCalledWith([-85, 45]);
    rerender(<MapLibreRouteMapClient {...props} showRouteEndpoints={false} />);
    expect(screen.queryByLabelText("Route start")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Route finish")).not.toBeInTheDocument();
    expect(mapMocks.markers).toHaveLength(2);
    rerender(
      <MapLibreRouteMapClient {...props} routePoints={[routePoints[0]]} />,
    );
    expect(mapMocks.markers).toHaveLength(2);
    rerender(<MapLibreRouteMapClient {...props} />);
    expect(screen.getByLabelText("Route finish")).toBeInTheDocument();
    unmount();
    mapMocks.markers.forEach((marker) =>
      expect(marker.remove).toHaveBeenCalledOnce(),
    );
  });

  it("keeps direction arrows above a colored route and updates its color without refitting", () => {
    const props = {
      routePoints: [
        { elapsed_seconds: 0, latitude: 45.01, longitude: -85.01 },
        { elapsed_seconds: 60, latitude: 45, longitude: -85 },
      ],
      ariaLabel: "Focused route map",
      emptyMessage: "No route",
    };
    const { rerender } = render(
      <MapLibreRouteMapClient {...props} routeColor="#f43f5e" />,
    );
    const route = mapMocks.sources.get("activity-route")!;
    expect(route.setData.mock.lastCall?.[0].features[0]).toMatchObject({
      properties: { color: "#f43f5e" },
      geometry: {
        coordinates: [
          [-85.01, 45.01],
          [-85, 45],
        ],
      },
    });
    expect(mapMocks.layers.get("activity-route-line")?.paint).toMatchObject({
      "line-color": ["get", "color"],
    });
    expect(mapMocks.layers.get("activity-route-arrows")).toMatchObject({
      source: "activity-route",
      layout: {
        "symbol-placement": "line",
        "text-field": "▶",
        "text-keep-upright": false,
      },
      paint: { "text-color": "#ffffff", "text-halo-color": ["get", "color"] },
    });
    const layers = [...mapMocks.layers.keys()];
    expect(layers.indexOf("activity-route-arrows")).toBeGreaterThan(
      layers.indexOf("activity-route-line"),
    );
    expect(
      mapMocks.sources.get("activity-route-overlays")?.setData.mock
        .lastCall?.[0].features,
    ).toEqual([]);
    rerender(<MapLibreRouteMapClient {...props} routeColor="#0060df" />);
    expect(route.setData.mock.lastCall?.[0].features[0].properties.color).toBe(
      "#0060df",
    );
    expect(mapMocks.fitBounds).toHaveBeenCalledOnce();
  });

  it("applies the latest highlight while previous source data is still loading", () => {
    const routePoints = [
      { elapsed_seconds: 0, latitude: 45, longitude: -85 },
      { elapsed_seconds: 60, latitude: 45.01, longitude: -85.01 },
    ];
    const props = {
      routePoints,
      ariaLabel: "Activity route map",
      emptyMessage: "No route",
    };
    const overlay = { id: "segment", points: routePoints, color: "#f43f5e" };
    const { rerender } = render(
      <MapLibreRouteMapClient {...props} overlays={[overlay]} />,
    );
    const source = mapMocks.sources.get("activity-route-overlays")!;
    expect(source.setData.mock.lastCall?.[0].features[0].properties.color).toBe(
      "#f43f5e",
    );
    mapMocks.styleLoaded = false;
    rerender(
      <MapLibreRouteMapClient
        {...props}
        overlays={[{ ...overlay, color: "#0060df" }]}
      />,
    );
    expect(source.setData.mock.lastCall?.[0].features[0].properties.color).toBe(
      "#0060df",
    );
    rerender(<MapLibreRouteMapClient {...props} overlays={[overlay]} />);
    expect(source.setData.mock.lastCall?.[0].features[0].properties.color).toBe(
      "#f43f5e",
    );
    expect(mapMocks.fitBounds).toHaveBeenCalledOnce();
  });

  it("preserves active hover listeners while dispatching current overlay callbacks", () => {
    const routePoints = [
      { elapsed_seconds: 0, latitude: 45, longitude: -85 },
      { elapsed_seconds: 60, latitude: 45.01, longitude: -85.01 },
    ];
    const onMouseEnter = vi.fn();
    const previousLeave = vi.fn();
    const onMouseLeave = vi.fn();
    const onClick = vi.fn();
    const overlay = {
      id: "segment",
      points: routePoints,
      color: "#f43f5e",
      onMouseEnter,
      onMouseLeave: previousLeave,
    };
    const props = {
      routePoints,
      ariaLabel: "Activity route map",
      emptyMessage: "No route",
    };
    const event = { features: [{ properties: { overlayId: "segment" } }] };
    const { rerender } = render(
      <MapLibreRouteMapClient {...props} overlays={[overlay]} />,
    );
    const enter = mapMocks.handlers.get("mouseenter")!.at(-1)!;
    const leave = mapMocks.handlers.get("mouseleave")!.at(-1)!;
    const click = mapMocks.handlers.get("click")!.at(-1)!;
    act(() => enter(event));
    expect(onMouseEnter).toHaveBeenCalledOnce();

    rerender(
      <MapLibreRouteMapClient
        {...props}
        overlays={[{ ...overlay, color: "#0060df", onMouseLeave, onClick }]}
      />,
    );
    expect(mapMocks.handlers.get("mouseleave")).toEqual([leave]);
    act(() => {
      click(event);
      leave();
    });
    expect(onClick).toHaveBeenCalledOnce();
    expect(onMouseLeave).toHaveBeenCalledOnce();
    expect(previousLeave).not.toHaveBeenCalled();
    expect(mapMocks.fitBounds).toHaveBeenCalledOnce();

    rerender(<MapLibreRouteMapClient {...props} overlays={[]} />);
    act(() => {
      click(event);
      leave();
    });
    expect(onClick).toHaveBeenCalledOnce();
    expect(onMouseLeave).toHaveBeenCalledOnce();
  });

  it("renders directional climb gradients and clears them without refitting the camera", async () => {
    const routePoints = [
      { elapsed_seconds: 0, latitude: 45, longitude: -85 },
      { elapsed_seconds: 60, latitude: 45.01, longitude: -85.01 },
    ];
    const props = {
      routePoints,
      ariaLabel: "Activity route map",
      emptyMessage: "No route",
    };
    const climbs = [
      { id: "climb-1", points: routePoints },
      { id: "climb-2", points: [...routePoints].reverse() },
    ];
    const { rerender } = render(<MapLibreRouteMapClient {...props} />);
    await waitFor(() => expect(mapMocks.fitBounds).toHaveBeenCalledTimes(1));
    expect(mapMocks.sources.has("activity-route-climbs")).toBe(false);
    rerender(<MapLibreRouteMapClient {...props} climbOverlays={climbs} />);
    expect(mapMocks.sourceOptions.get("activity-route-climbs")).toMatchObject({
      lineMetrics: true,
    });
    expect(
      mapMocks.layers.get("activity-route-climbs-line")?.paint,
    ).toMatchObject({
      "line-gradient": [
        "interpolate",
        ["linear"],
        ["line-progress"],
        0,
        "#fb923c",
        0.5,
        "#f43f5e",
        1,
        "#a855f7",
      ],
      "line-width": 4,
      "line-offset": 7,
    });
    expect(
      mapMocks.layers.get("activity-route-climbs-arrows")?.layout,
    ).toMatchObject({
      "symbol-placement": "line",
      "icon-keep-upright": false,
      "icon-rotation-alignment": "map",
      "icon-offset": [0, 7],
    });
    const source = mapMocks.sources.get("activity-route-climbs")!;
    const coordinates = source.setData.mock.lastCall?.[0].features.map(
      (feature: any) => feature.geometry.coordinates,
    );
    expect(coordinates).toEqual([
      [
        [-85, 45],
        [-85.01, 45.01],
      ],
      [
        [-85.01, 45.01],
        [-85, 45],
      ],
    ]);
    const arrow = mapMocks.images.get("climb-direction-arrow");
    expect(arrow).toMatchObject({ width: 16, height: 16 });
    expect(arrow.data.some((value: number) => value !== 0)).toBe(true);
    await act(async () => {
      document.documentElement.setAttribute("data-theme", "dark");
    });
    await act(async () => {
      mapMocks.sources.clear();
      mapMocks.layers.clear();
      mapMocks.images.clear();
      mapMocks.handlers.get("style.load")?.forEach((handler) => handler());
    });
    expect(mapMocks.sources.has("activity-route-climbs")).toBe(true);
    expect(mapMocks.images.has("climb-direction-arrow")).toBe(true);
    rerender(<MapLibreRouteMapClient {...props} climbOverlays={[]} />);
    expect(
      mapMocks.sources.get("activity-route-climbs")?.setData.mock.lastCall?.[0]
        .features,
    ).toEqual([]);
    expect(mapMocks.fitBounds).toHaveBeenCalledTimes(1);
  });

  it("returns from an explicit basemap to the current theme without refitting the camera", async () => {
    document.documentElement.setAttribute("data-theme", "dark");
    const props = {
      routePoints: [
        { elapsed_seconds: 0, latitude: 45, longitude: -85 },
        { elapsed_seconds: 60, latitude: 45.01, longitude: -85.01 },
      ],
      ariaLabel: "Segment race viewer map",
      emptyMessage: "No route",
    };
    const { rerender } = render(
      <MapLibreRouteMapClient {...props} selectedBasemap="default" />,
    );
    await waitFor(() => expect(mapMocks.fitBounds).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(mapMocks.setStyle).toHaveBeenLastCalledWith(
        "/map-styles/fiord-v1.json",
      ),
    );

    rerender(<MapLibreRouteMapClient {...props} selectedBasemap="street" />);
    await waitFor(() =>
      expect(mapMocks.setStyle).toHaveBeenLastCalledWith(
        "https://tiles.openfreemap.org/styles/positron",
      ),
    );

    rerender(<MapLibreRouteMapClient {...props} selectedBasemap="default" />);
    await waitFor(() =>
      expect(mapMocks.setStyle).toHaveBeenLastCalledWith(
        "/map-styles/fiord-v1.json",
      ),
    );
    await act(async () => {
      mapMocks.sources.clear();
      mapMocks.layers.clear();
      mapMocks.handlers.get("style.load")?.forEach((handler) => handler());
    });
    expect(mapMocks.layers.get("activity-route-line")?.paint).toEqual(
      expect.objectContaining({ "line-color": ["get", "color"] }),
    );
    expect(mapMocks.fitBounds).toHaveBeenCalledTimes(1);
    expect(mapMocks.jumpTo).toHaveBeenCalled();
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
    expect(mapMocks.sources.has("cycling-trails")).toBe(false);

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

  it("preserves user zoom during follow mode without fighting the zoom gesture", async () => {
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
        followViewportPreserveUserZoom
        ariaLabel="Segment comparison map"
        emptyMessage="Segment route geometry is not available yet."
        fitBoundsPadding={24}
        fitBoundsMaxZoom={18}
      />,
    );

    await waitFor(() => {
      expect(mapMocks.jumpTo).toHaveBeenCalledWith(
        expect.objectContaining({
          zoom: 19,
        }),
      );
    });

    mapMocks.zoom = 16;
    mapMocks.handlers.get("zoomstart")?.forEach((handler) => {
      handler({ originalEvent: new Event("wheel") });
    });

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
        followViewportPreserveUserZoom
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

    expect(mapMocks.easeTo).not.toHaveBeenCalled();

    mapMocks.handlers.get("zoomend")?.forEach((handler) => {
      handler({ originalEvent: new Event("wheel") });
    });

    rerender(
      <MapLibreRouteMapClient
        routePoints={routePoints}
        movingMarkers={[
          {
            id: "1",
            point: routePoints[2],
            color: "#0f766e",
          },
        ]}
        followViewport={{ point: routePoints[2], zoom: 17 }}
        followViewportPreserveUserZoom
        ariaLabel="Segment comparison map"
        emptyMessage="Segment route geometry is not available yet."
        fitBoundsPadding={24}
        fitBoundsMaxZoom={18}
      />,
    );

    await waitFor(() => {
      expect(mapMocks.easeTo).toHaveBeenLastCalledWith(
        expect.objectContaining({
          zoom: 16,
        }),
      );
    });
  });

  it("keeps a manually tighter follow zoom instead of widening it later", async () => {
    const routePoints = [
      { elapsed_seconds: 0, latitude: 45.0, longitude: -122.0 },
      { elapsed_seconds: 120, latitude: 45.004, longitude: -121.996 },
      { elapsed_seconds: 240, latitude: 45.008, longitude: -121.992 },
    ];

    const { rerender } = render(
      <MapLibreRouteMapClient
        routePoints={routePoints}
        followViewport={{ point: routePoints[0], zoom: 19 }}
        followViewportPreserveUserZoom
        ariaLabel="Segment comparison map"
        emptyMessage="Segment route geometry is not available yet."
        fitBoundsPadding={24}
        fitBoundsMaxZoom={18}
      />,
    );

    await waitFor(() => {
      expect(mapMocks.jumpTo).toHaveBeenCalledWith(
        expect.objectContaining({
          zoom: 19,
        }),
      );
    });

    mapMocks.zoom = 20;
    mapMocks.handlers.get("zoomstart")?.forEach((handler) => {
      handler({ originalEvent: new Event("wheel") });
    });
    mapMocks.handlers.get("zoomend")?.forEach((handler) => {
      handler({ originalEvent: new Event("wheel") });
    });

    rerender(
      <MapLibreRouteMapClient
        routePoints={routePoints}
        followViewport={{ point: routePoints[2], zoom: 17 }}
        followViewportPreserveUserZoom
        ariaLabel="Segment comparison map"
        emptyMessage="Segment route geometry is not available yet."
        fitBoundsPadding={24}
        fitBoundsMaxZoom={18}
      />,
    );

    await waitFor(() => {
      expect(mapMocks.easeTo).toHaveBeenLastCalledWith(
        expect.objectContaining({
          zoom: 20,
        }),
      );
    });
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
      { points: [number, number][] } | undefined;

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
