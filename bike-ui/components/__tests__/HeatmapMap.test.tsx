import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import HeatmapMap from "../HeatmapMap";
import type { HeatmapMetadata } from "../../lib/heatmaps";
import { DEFAULT_HEATMAP_ZOOM } from "../../lib/heatmapCamera";
import { locateMapRegion } from "../../lib/mapRegions";

const mocks = vi.hoisted(() => ({
  theme: "light" as "light" | "dark",
  create: vi.fn(),
  navigationOptions: vi.fn(),
  jumpTo: vi.fn(),
  getCenter: vi.fn(() => ({
    lng: -85,
    lat: 45,
    wrap() {
      return this;
    },
  })),
  setTiles: vi.fn(),
  setStyle: vi.fn(),
  setPaintProperty: vi.fn(),
  addLayer: vi.fn(),
  zoneSetData: vi.fn(),
  clusterExpansionZoom: vi.fn(),
  zoneSourceDefinition: undefined as unknown,
  fitBounds: vi.fn(),
  flyTo: vi.fn(),
  easeTo: vi.fn(),
  getZoom: vi.fn(() => 6),
  getCurrentPosition: vi.fn(),
  layers: new Set<string>(),
  sources: new Map<
    string,
    {
      setTiles?: ReturnType<typeof vi.fn>;
      setData?: ReturnType<typeof vi.fn>;
      getClusterExpansionZoom?: ReturnType<typeof vi.fn>;
    }
  >(),
  handlers: new Map<string, (event?: unknown) => void>(),
}));
vi.mock("../../lib/useBikeTheme", () => ({ useBikeTheme: () => mocks.theme }));
vi.mock("../../lib/mapRegions", async (original) => ({
  ...(await original<typeof import("../../lib/mapRegions")>()),
  locateMapRegion: vi.fn(),
}));
vi.mock("maplibre-gl", () => ({
  setWorkerUrl: vi.fn(),
  Map: class {
    container: HTMLElement;
    constructor(options: { container: HTMLElement; center: number[] }) {
      mocks.create(options);
      this.container = options.container;
      mocks.getCenter.mockReturnValue({
        lng: options.center[0],
        lat: options.center[1],
        wrap() {
          return this;
        },
      });
    }
    addControl(control: { onAdd?: (map: unknown) => HTMLElement }) {
      if (control.onAdd) this.container.append(control.onAdd(this));
    }
    on(
      event: string,
      layerOrHandler: string | ((event?: unknown) => void),
      handler?: (event?: unknown) => void,
    ) {
      if (typeof layerOrHandler === "string") {
        mocks.handlers.set(`${event}:${layerOrHandler}`, handler!);
      } else {
        const previous = mocks.handlers.get(event);
        mocks.handlers.set(event, (value) => {
          previous?.(value);
          layerOrHandler(value);
        });
      }
    }
    getSource(id: string) {
      return mocks.sources.get(id);
    }
    getLayer(id: string) {
      return mocks.layers.has(id);
    }
    addSource(id: string, definition: unknown) {
      if (id === "personal-heatmap-zones") {
        mocks.zoneSourceDefinition = definition;
        mocks.sources.set(id, {
          setData: mocks.zoneSetData,
          getClusterExpansionZoom: mocks.clusterExpansionZoom,
        });
      } else {
        mocks.sources.set(id, { setTiles: mocks.setTiles });
      }
    }
    removeSource(id: string) {
      mocks.sources.delete(id);
    }
    addLayer(layer: { id: string }) {
      mocks.layers.add(layer.id);
      mocks.addLayer(layer);
    }
    removeLayer(id: string) {
      mocks.layers.delete(id);
    }
    getStyle() {
      return { layers: [{ id: "labels", type: "symbol" }] };
    }
    // Outstanding zoom tiles make isStyleLoaded false even after style.load.
    isStyleLoaded() {
      return false;
    }
    once() {}
    off() {}
    fitBounds = mocks.fitBounds;
    flyTo = mocks.flyTo;
    easeTo = mocks.easeTo;
    getZoom = mocks.getZoom;
    getCenter = mocks.getCenter;
    jumpTo = mocks.jumpTo;
    setStyle = mocks.setStyle;
    setPaintProperty = mocks.setPaintProperty;
    getCanvas() {
      return { style: { cursor: "" } };
    }
    resize() {}
    remove() {}
  },
  NavigationControl: class {
    constructor(options: unknown) {
      mocks.navigationOptions(options);
    }
    onAdd() {
      const container = document.createElement("div");
      for (const name of ["Zoom in", "Zoom out"]) {
        const button = document.createElement("button");
        button.setAttribute("aria-label", name);
        container.append(button);
      }
      return container;
    }
  },
  AttributionControl: class {},
}));

const metadata: HeatmapMetadata = {
  revision: "1",
  filters: {},
  ready: 25,
  pending: 0,
  failed: 0,
  skipped: 0,
  bounds: [-85.7, 44.6, -85.5, 44.8],
  preparing: false,
  min_zoom: 0,
  max_zoom: 18,
  tile_size: 512,
  style_version: "heatmap-v2",
};
const zones = [
  { longitude: -85, latitude: 45 },
  { longitude: -85.01, latitude: 45.01 },
];
const props = {
  metadata,
  tileUrl: "/heatmap-tiles/{z}/{x}/{y}.png?revision=1",
  zones,
  onStale: vi.fn(),
  onTileError: vi.fn(),
  view: { onChange: vi.fn(), onRequest: vi.fn(), onError: vi.fn() },
  onMapMoveStart: vi.fn(),
};
function loadStyle() {
  act(() => mocks.handlers.get("style.load")!());
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.theme = "light";
  mocks.layers.clear();
  mocks.sources.clear();
  mocks.zoneSourceDefinition = undefined;
  mocks.handlers.clear();
  vi.mocked(locateMapRegion).mockReset();
  mocks.getZoom.mockReturnValue(6);
  vi.stubGlobal("navigator", {
    geolocation: { getCurrentPosition: mocks.getCurrentPosition },
  });
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
});

describe("heatmap overlay lifecycle", () => {
  it("clusters route centers into low-zoom zone markers", () => {
    render(<HeatmapMap {...props} />);
    loadStyle();
    expect(mocks.zoneSourceDefinition).toMatchObject({
      type: "geojson",
      cluster: true,
      clusterRadius: 64,
      clusterMaxZoom: 6,
      data: {
        features: [
          expect.objectContaining({
            geometry: {
              type: "Point",
              coordinates: [-85, 45],
            },
          }),
          expect.objectContaining({
            geometry: {
              type: "Point",
              coordinates: [-85.01, 45.01],
            },
          }),
        ],
      },
    });
    expect(mocks.addLayer).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "personal-heatmap-zone-circles",
        maxzoom: 7,
        filter: ["has", "point_count"],
        paint: expect.objectContaining({
          "circle-color": [
            "step",
            ["get", "point_count"],
            "#1265cc",
            10,
            "#0d4fa4",
            50,
            "#083b83",
          ],
        }),
      }),
    );
    expect(mocks.addLayer).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "personal-heatmap-zone-counts",
        maxzoom: 7,
      }),
    );
    expect(mocks.addLayer).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "personal-heatmap-single-zone-circles",
        maxzoom: 6,
        paint: expect.objectContaining({ "circle-color": "#1265cc" }),
      }),
    );
  });
  it("zooms into a clicked cluster or individual route zone", async () => {
    mocks.clusterExpansionZoom.mockResolvedValue(7);
    render(<HeatmapMap {...props} />);
    loadStyle();
    const clusterClick = mocks.handlers.get(
      "click:personal-heatmap-zone-circles",
    ) as (event: unknown) => void;
    await act(async () => {
      clusterClick({
        features: [
          {
            geometry: { type: "Point", coordinates: [-85, 45] },
            properties: { cluster_id: 42 },
          },
        ],
      });
      await Promise.resolve();
    });
    expect(mocks.clusterExpansionZoom).toHaveBeenCalledWith(42);
    expect(mocks.easeTo).toHaveBeenCalledWith({
      center: [-85, 45],
      zoom: 8,
      duration: 600,
    });

    mocks.easeTo.mockClear();
    const singleZoneClick = mocks.handlers.get(
      "click:personal-heatmap-single-zone-circles",
    ) as (event: unknown) => void;
    singleZoneClick({
      features: [
        {
          geometry: { type: "Point", coordinates: [-84, 44] },
          properties: {},
        },
      ],
    });
    expect(mocks.easeTo).toHaveBeenCalledWith({
      center: [-84, 44],
      zoom: 8,
      duration: 600,
    });
  });
  it("updates zone marker positions without recreating the map", () => {
    const { rerender } = render(<HeatmapMap {...props} />);
    loadStyle();
    rerender(
      <HeatmapMap {...props} zones={[{ longitude: -84, latitude: 44 }]} />,
    );
    expect(mocks.zoneSetData).toHaveBeenCalledWith({
      type: "FeatureCollection",
      features: [
        {
          type: "Feature",
          properties: {},
          geometry: { type: "Point", coordinates: [-84, 44] },
        },
      ],
    });
    expect(mocks.create).toHaveBeenCalledOnce();
  });
  it("clears tile errors when the map is moved to retry affected areas", () => {
    render(<HeatmapMap {...props} />);
    act(() => mocks.handlers.get("movestart")!());
    expect(props.onMapMoveStart).toHaveBeenCalledOnce();
  });
  it("automatically centers on the user's location before routes arrive", async () => {
    const { rerender } = render(<HeatmapMap {...props} zones={[]} />);
    expect(mocks.getCurrentPosition).toHaveBeenCalledOnce();
    await act(async () =>
      mocks.getCurrentPosition.mock.calls[0][0]({
        coords: { longitude: -122.33, latitude: 47.61 },
      }),
    );
    expect(mocks.jumpTo).toHaveBeenCalledWith({
      center: [-122.33, 47.61],
      zoom: DEFAULT_HEATMAP_ZOOM,
    });
    rerender(<HeatmapMap {...props} />);
    expect(mocks.jumpTo).toHaveBeenCalledOnce();
    expect(mocks.getCurrentPosition).toHaveBeenCalledOnce();
    expect(mocks.fitBounds).not.toHaveBeenCalled();
  });
  it.each([1, 2, 3])(
    "falls back to local routes when automatic location fails with code %s",
    async (code) => {
      render(<HeatmapMap {...props} />);
      await act(async () =>
        mocks.getCurrentPosition.mock.calls[0][1]({ code }),
      );
      expect(mocks.jumpTo).toHaveBeenCalledWith({
        center: [-85, 45],
        zoom: DEFAULT_HEATMAP_ZOOM,
      });
      expect(mocks.fitBounds).not.toHaveBeenCalled();
      expect(props.view.onError).not.toHaveBeenCalled();
    },
  );
  it("falls back to local routes when browser geolocation is unavailable", async () => {
    vi.stubGlobal("navigator", {});
    render(<HeatmapMap {...props} />);
    await act(async () => {});
    expect(mocks.jumpTo).toHaveBeenCalledWith({
      center: [-85, 45],
      zoom: DEFAULT_HEATMAP_ZOOM,
    });
    expect(mocks.getCurrentPosition).not.toHaveBeenCalled();
  });
  it("waits for delayed routes without saving resize events as a chosen camera", async () => {
    const { rerender } = render(<HeatmapMap {...props} zones={[]} />);
    mocks.getZoom.mockReturnValue(2);
    act(() => {
      mocks.handlers.get("movestart")!({});
      mocks.handlers.get("moveend")!({});
    });
    expect(props.view.onChange).not.toHaveBeenCalled();
    await act(async () =>
      mocks.getCurrentPosition.mock.calls[0][1]({ code: 1 }),
    );
    rerender(<HeatmapMap {...props} />);
    expect(mocks.jumpTo).toHaveBeenCalledWith({
      center: [-85, 45],
      zoom: DEFAULT_HEATMAP_ZOOM,
    });
    expect(mocks.create).toHaveBeenCalledOnce();
  });
  it("preserves a view the user moves to before location and routes arrive", async () => {
    const { rerender } = render(<HeatmapMap {...props} zones={[]} />);
    act(() => {
      mocks.handlers.get("movestart")!({
        originalEvent: new MouseEvent("mousedown"),
      });
      mocks.handlers.get("moveend")!({});
    });
    expect(props.view.onChange).toHaveBeenCalledOnce();
    await act(async () =>
      mocks.getCurrentPosition.mock.calls[0][0]({
        coords: { longitude: -122.33, latitude: 47.61 },
      }),
    );
    rerender(<HeatmapMap {...props} />);
    expect(mocks.jumpTo).not.toHaveBeenCalled();
  });
  it("ignores automatic location when Full is selected during detection", async () => {
    const { rerender } = render(<HeatmapMap {...props} />);
    const success = mocks.getCurrentPosition.mock.calls[0][0];
    rerender(
      <HeatmapMap
        {...props}
        view={{ ...props.view, request: { action: "full", sequence: 1 } }}
      />,
    );
    await act(async () =>
      success({ coords: { longitude: -122.33, latitude: 47.61 } }),
    );
    expect(mocks.jumpTo).not.toHaveBeenCalled();
    expect(mocks.fitBounds).toHaveBeenCalledOnce();
  });
  it("ignores an automatic location result after the map is closed", async () => {
    const { unmount } = render(<HeatmapMap {...props} />);
    const success = mocks.getCurrentPosition.mock.calls[0][0];
    unmount();
    await act(async () =>
      success({ coords: { longitude: -122.33, latitude: 47.61 } }),
    );
    expect(mocks.jumpTo).not.toHaveBeenCalled();
  });
  it("places GPS immediately below zoom out with no compass", () => {
    render(<HeatmapMap {...props} />);
    const buttons = screen
      .getByRole("button", { name: "Use my location" })
      .parentElement!.querySelectorAll("button");
    expect(
      [...buttons].map((button) => button.getAttribute("aria-label")),
    ).toEqual(["Zoom in", "Zoom out", "Use my location"]);
    expect(mocks.navigationOptions).toHaveBeenCalledWith({
      showCompass: false,
    });
    fireEvent.click(buttons[2]);
    expect(props.view.onRequest).toHaveBeenCalledWith("location");
  });
  it("recenters on the requested location at the local default zoom", async () => {
    const { rerender } = render(<HeatmapMap {...props} />);
    expect(mocks.getCurrentPosition).toHaveBeenCalledOnce();
    rerender(
      <HeatmapMap
        {...props}
        view={{ ...props.view, request: { action: "location", sequence: 1 } }}
      />,
    );
    expect(mocks.getCurrentPosition).toHaveBeenCalledWith(
      expect.any(Function),
      expect.any(Function),
      { enableHighAccuracy: false, maximumAge: 300_000, timeout: 10_000 },
    );
    const onSuccess = mocks.getCurrentPosition.mock
      .calls[1][0] as PositionCallback;
    await act(async () =>
      onSuccess({
        coords: { longitude: -85.6, latitude: 44.7 },
      } as GeolocationPosition),
    );
    expect(mocks.flyTo).toHaveBeenCalledWith({
      center: [-85.6, 44.7],
      zoom: DEFAULT_HEATMAP_ZOOM,
      duration: 900,
    });
    expect(mocks.fitBounds).not.toHaveBeenCalled();
  });
  it("fits the entire current state, including both Michigan peninsulas", async () => {
    vi.mocked(locateMapRegion).mockResolvedValue({
      name: "Michigan",
      bounds: [-90.42, 41.7, -82.12, 48.3],
    });
    render(
      <HeatmapMap
        {...props}
        view={{ ...props.view, request: { action: "region", sequence: 1 } }}
      />,
    );
    await act(async () =>
      mocks.getCurrentPosition.mock.calls[0][0]({
        coords: { longitude: -85.62, latitude: 44.76 },
      }),
    );
    expect(locateMapRegion).toHaveBeenCalledWith(
      [-85.62, 44.76],
      expect.any(AbortSignal),
    );
    expect(mocks.fitBounds).toHaveBeenCalledWith(
      [
        [-90.42, 41.7],
        [-82.12, 48.3],
      ],
      { padding: 50, maxZoom: 14, duration: 0 },
    );
  });
  it("fits the latest filtered bounds only when Full is selected", () => {
    const { rerender } = render(<HeatmapMap {...props} />);
    loadStyle();
    rerender(
      <HeatmapMap
        {...props}
        metadata={{ ...metadata, bounds: [-125, 30, -70, 50] }}
        view={{ ...props.view, request: { action: "full", sequence: 1 } }}
      />,
    );
    expect(mocks.fitBounds).toHaveBeenCalledWith(
      [
        [-125, 30],
        [-70, 50],
      ],
      { padding: 50, maxZoom: 14, duration: 0 },
    );
    expect(mocks.getCurrentPosition).toHaveBeenCalledOnce();
  });
  it("restores a saved camera before considering local routes", () => {
    const camera = { longitude: -122, latitude: 47, zoom: 9 };
    render(<HeatmapMap {...props} view={{ ...props.view, camera }} />);
    expect(mocks.create).toHaveBeenCalledWith(
      expect.objectContaining({ center: [-122, 47], zoom: 9 }),
    );
    expect(mocks.jumpTo).not.toHaveBeenCalledWith(
      expect.objectContaining({ zoom: DEFAULT_HEATMAP_ZOOM }),
    );
    expect(mocks.getCurrentPosition).not.toHaveBeenCalled();
  });
  it("restores a different saved camera when browser navigation changes it", () => {
    const { rerender } = render(
      <HeatmapMap
        {...props}
        view={{
          ...props.view,
          camera: { longitude: -122, latitude: 47, zoom: 9 },
        }}
      />,
    );
    mocks.jumpTo.mockClear();
    rerender(
      <HeatmapMap
        {...props}
        view={{
          ...props.view,
          camera: { longitude: -87, latitude: 44, zoom: 7 },
        }}
      />,
    );
    expect(mocks.jumpTo).toHaveBeenCalledWith({ center: [-87, 44], zoom: 7 });
  });
  it("leaves the camera unchanged when Full has no ready route bounds", () => {
    render(
      <HeatmapMap
        {...props}
        metadata={{ ...metadata, ready: 0, bounds: null }}
        view={{ ...props.view, request: { action: "full", sequence: 1 } }}
      />,
    );
    expect(mocks.fitBounds).not.toHaveBeenCalled();
    expect(mocks.getCurrentPosition).not.toHaveBeenCalled();
  });
  it("reports location denial without moving the camera", async () => {
    render(
      <HeatmapMap
        {...props}
        view={{ ...props.view, request: { action: "region", sequence: 1 } }}
      />,
    );
    await act(async () =>
      mocks.getCurrentPosition.mock.calls[0][1]({ code: 1 }),
    );
    expect(props.view.onError).toHaveBeenCalledWith(
      expect.stringContaining("denied"),
    );
    expect(mocks.fitBounds).not.toHaveBeenCalled();
    expect(mocks.flyTo).not.toHaveBeenCalled();
  });
  it("keeps the camera when state resolution fails", async () => {
    vi.mocked(locateMapRegion).mockRejectedValue(
      new Error("Could not determine your state."),
    );
    render(
      <HeatmapMap
        {...props}
        view={{ ...props.view, request: { action: "region", sequence: 1 } }}
      />,
    );
    await act(async () =>
      mocks.getCurrentPosition.mock.calls[0][0]({
        coords: { longitude: 0, latitude: 0 },
      }),
    );
    expect(props.view.onError).toHaveBeenCalledWith(
      "Could not determine your state.",
    );
    expect(mocks.fitBounds).not.toHaveBeenCalled();
  });
  it("ignores a location result superseded by Full", async () => {
    const { rerender } = render(
      <HeatmapMap
        {...props}
        view={{ ...props.view, request: { action: "location", sequence: 1 } }}
      />,
    );
    const success = mocks.getCurrentPosition.mock.calls[0][0];
    rerender(
      <HeatmapMap
        {...props}
        view={{ ...props.view, request: { action: "full", sequence: 2 } }}
      />,
    );
    await act(async () =>
      success({ coords: { longitude: -85, latitude: 45 } }),
    );
    expect(mocks.flyTo).not.toHaveBeenCalled();
    expect(mocks.fitBounds).toHaveBeenCalledOnce();
  });
  it("keeps loaded tiles on an unchanged metadata refresh", () => {
    const { rerender } = render(<HeatmapMap {...props} />);
    loadStyle();
    rerender(<HeatmapMap {...props} metadata={{ ...metadata, pending: 4 }} />);
    expect(mocks.setTiles).not.toHaveBeenCalled();
    expect(mocks.fitBounds).not.toHaveBeenCalled();
  });
  it("applies a new revision while zoom tiles are still loading", () => {
    const { rerender } = render(<HeatmapMap {...props} />);
    loadStyle();
    rerender(
      <HeatmapMap
        {...props}
        tileUrl="/heatmap-tiles/{z}/{x}/{y}.png?revision=2"
      />,
    );
    expect(mocks.setTiles).toHaveBeenCalledWith([
      `${window.location.origin}/heatmap-tiles/{z}/{x}/{y}.png?revision=2`,
    ]);
    expect(mocks.create).toHaveBeenCalledOnce();
  });
  it("recolors loaded tiles without reloading them or moving the camera", () => {
    const { rerender } = render(<HeatmapMap {...props} />);
    loadStyle();
    rerender(<HeatmapMap {...props} paletteId="orange" />);
    expect(mocks.setPaintProperty).toHaveBeenCalledWith(
      "personal-heatmap",
      "raster-hue-rotate",
      180,
    );
    expect(mocks.setTiles).not.toHaveBeenCalled();
    expect(mocks.create).toHaveBeenCalledOnce();
    expect(mocks.fitBounds).not.toHaveBeenCalled();
    // A theme change rebuilds the layer using the current color selection.
    mocks.theme = "dark";
    mocks.sources.clear();
    mocks.layers.clear();
    rerender(<HeatmapMap {...props} paletteId="orange" />);
    loadStyle();
    const rasterLayers = mocks.addLayer.mock.calls
      .map(([layer]) => layer)
      .filter((layer) => layer.id === "personal-heatmap");
    expect(rasterLayers.at(-1)).toMatchObject({
      paint: {
        "raster-hue-rotate": 180,
        "raster-opacity": 1,
      },
    });
    expect(mocks.fitBounds).not.toHaveBeenCalled();
  });
  it("restores the latest tiles after a theme style loads without refitting", () => {
    const { rerender } = render(<HeatmapMap {...props} />);
    loadStyle();
    mocks.theme = "dark";
    mocks.sources.clear();
    mocks.layers.clear();
    rerender(
      <HeatmapMap
        {...props}
        tileUrl="/heatmap-tiles/{z}/{x}/{y}.png?revision=2"
      />,
    );
    expect(mocks.setStyle).toHaveBeenCalledWith("/map-styles/fiord-v1.json");
    expect(mocks.sources.size).toBe(0);
    loadStyle();
    expect(mocks.sources.has("personal-heatmap")).toBe(true);
    expect(mocks.layers.has("personal-heatmap")).toBe(true);
    expect(mocks.create).toHaveBeenCalledOnce();
    expect(mocks.fitBounds).not.toHaveBeenCalled();
  });
  it("removes old filtered heat while zoom tiles are loading", () => {
    const { rerender } = render(<HeatmapMap {...props} />);
    loadStyle();
    rerender(<HeatmapMap {...props} tileUrl={undefined} />);
    expect(mocks.sources.has("personal-heatmap")).toBe(false);
    expect(mocks.layers.has("personal-heatmap")).toBe(false);
  });
});
