import { act, render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import HeatmapMap from "../HeatmapMap";
import type { HeatmapMetadata } from "../../lib/heatmaps";

const mocks = vi.hoisted(() => ({
  theme: "light" as "light" | "dark",
  create: vi.fn(),
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
vi.mock("maplibre-gl", () => ({
  default: {
    Map: class {
      constructor() {
        mocks.create();
      }
      addControl() {}
      on(
        event: string,
        layerOrHandler: string | ((event?: unknown) => void),
        handler?: (event?: unknown) => void,
      ) {
        if (typeof layerOrHandler === "string") {
          mocks.handlers.set(`${event}:${layerOrHandler}`, handler!);
        } else {
          mocks.handlers.set(event, layerOrHandler);
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
      setStyle = mocks.setStyle;
      setPaintProperty = mocks.setPaintProperty;
      getCanvas() {
        return { style: { cursor: "" } };
      }
      resize() {}
      remove() {}
    },
    NavigationControl: class {},
    AttributionControl: class {},
  },
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
  locationRequest: 0,
  onLocationError: vi.fn(),
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
  it("recenters on the requested location at a closer zoom", () => {
    const { rerender } = render(<HeatmapMap {...props} />);
    expect(mocks.getCurrentPosition).not.toHaveBeenCalled();
    rerender(<HeatmapMap {...props} locationRequest={1} />);
    expect(mocks.getCurrentPosition).toHaveBeenCalledWith(
      expect.any(Function),
      expect.any(Function),
      { enableHighAccuracy: false, maximumAge: 300_000, timeout: 10_000 },
    );
    const onSuccess = mocks.getCurrentPosition.mock
      .calls[0][0] as PositionCallback;
    act(() =>
      onSuccess({
        coords: { longitude: -85.6, latitude: 44.7 },
      } as GeolocationPosition),
    );
    expect(mocks.flyTo).toHaveBeenCalledWith({
      center: [-85.6, 44.7],
      zoom: 12,
      duration: 900,
    });
    expect(mocks.fitBounds).not.toHaveBeenCalled();
  });
  it("keeps loaded tiles on an unchanged metadata refresh", () => {
    const { rerender } = render(<HeatmapMap {...props} />);
    loadStyle();
    rerender(<HeatmapMap {...props} metadata={{ ...metadata, pending: 4 }} />);
    expect(mocks.setTiles).not.toHaveBeenCalled();
    expect(mocks.fitBounds).toHaveBeenCalledOnce();
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
    expect(mocks.fitBounds).toHaveBeenCalledOnce();
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
    expect(mocks.fitBounds).toHaveBeenCalledOnce();
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
    expect(mocks.fitBounds).toHaveBeenCalledOnce();
  });
  it("removes old filtered heat while zoom tiles are loading", () => {
    const { rerender } = render(<HeatmapMap {...props} />);
    loadStyle();
    rerender(<HeatmapMap {...props} tileUrl={undefined} />);
    expect(mocks.sources.has("personal-heatmap")).toBe(false);
    expect(mocks.layers.has("personal-heatmap")).toBe(false);
  });
});
