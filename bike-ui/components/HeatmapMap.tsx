"use client";

import * as maplibregl from "../lib/maplibre";
import { type GeoJSONSource, type RasterTileSource } from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { useEffect, useRef } from "react";
import type { FeatureCollection, Point } from "geojson";
import { useBikeTheme } from "../lib/useBikeTheme";
import type { HeatmapMetadata, HeatmapZones } from "../lib/heatmaps";
import {
  DEFAULT_HEATMAP_ZOOM,
  HEATMAP_LOADING_CAMERA,
  HEATMAP_ZONE_LAYERS,
  HEATMAP_ZONES_SOURCE,
  localRidingCenter,
  type HeatmapView,
} from "../lib/heatmapCamera";
import { useHeatmapCamera } from "../lib/useHeatmapCamera";
import HeatmapNavigationControl from "./HeatmapNavigationControl";
import {
  heatmapPalette,
  heatmapPalettePaint,
  type HeatmapPaletteId,
} from "../lib/heatmapColors";

const SOURCE = "personal-heatmap";
const ZONES_SOURCE = HEATMAP_ZONES_SOURCE;
const ZONES_CIRCLES = HEATMAP_ZONE_LAYERS[0];
const ZONES_COUNTS = "personal-heatmap-zone-counts";
const SINGLE_ZONE_CIRCLES = HEATMAP_ZONE_LAYERS[1];
const SINGLE_ZONE_COUNTS = "personal-heatmap-single-zone-counts";
const styleUrl = (theme: string) =>
  theme === "dark"
    ? "/map-styles/fiord-v1.json"
    : "/map-styles/route-light-v1.json";

function zoneFeatures(zones: HeatmapZones["zones"]): FeatureCollection<Point> {
  return {
    type: "FeatureCollection",
    features: zones.map((zone) => ({
      type: "Feature",
      properties: {},
      geometry: {
        type: "Point",
        coordinates: [zone.longitude, zone.latitude],
      },
    })),
  };
}

export default function HeatmapMap({
  metadata,
  tileUrl,
  zones,
  paletteId = "blue",
  onStale,
  onTileError,
  view,
  onMapMoveStart,
}: {
  metadata?: HeatmapMetadata;
  tileUrl?: string;
  zones: HeatmapZones["zones"];
  paletteId?: HeatmapPaletteId;
  onStale: () => void;
  onTileError: (message: string) => void;
  view: HeatmapView;
  onMapMoveStart: () => void;
}) {
  const container = useRef<HTMLDivElement>(null);
  const map = useRef<maplibregl.Map | null>(null);
  const current = useRef({
    metadata,
    tileUrl,
    zones,
    paletteId,
    onStale,
    onTileError,
    view,
    onMapMoveStart,
  });
  useEffect(() => {
    current.current = {
      metadata,
      tileUrl,
      zones,
      paletteId,
      onStale,
      onTileError,
      view,
      onMapMoveStart,
    };
  }, [
    metadata,
    tileUrl,
    zones,
    paletteId,
    onStale,
    onTileError,
    view,
    onMapMoveStart,
  ]);
  const zoneInteractionBound = useRef(false);
  const styleReady = useRef(false);
  const syncOverlay = useRef(() => {});
  const syncZones = useRef(() => {});
  const theme = useBikeTheme();
  const appliedTheme = useRef(theme);

  useEffect(() => {
    const camera = current.current.view.camera;
    const localCenter = localRidingCenter(current.current.zones);
    const instance = new maplibregl.Map({
      container: container.current!,
      style: styleUrl(appliedTheme.current),
      center: camera
        ? [camera.longitude, camera.latitude]
        : (localCenter ?? [
            HEATMAP_LOADING_CAMERA.longitude,
            HEATMAP_LOADING_CAMERA.latitude,
          ]),
      zoom:
        camera?.zoom ??
        (localCenter ? DEFAULT_HEATMAP_ZOOM : HEATMAP_LOADING_CAMERA.zoom),
      minZoom: 0,
      maxZoom: 18,
      attributionControl: false,
    });
    map.current = instance;
    instance.addControl(
      new HeatmapNavigationControl(() =>
        current.current.view.onRequest("location"),
      ),
      "bottom-right",
    );
    instance.addControl(
      new maplibregl.AttributionControl({ compact: false }),
      "bottom-left",
    );
    let appliedTileUrl: string | undefined;
    let appliedPaletteId: HeatmapPaletteId | undefined;
    const overlay = () => {
      if (!styleReady.current) return;
      const { tileUrl, paletteId } = current.current;
      const source = instance.getSource(SOURCE) as RasterTileSource | undefined;
      if (!tileUrl) {
        if (instance.getLayer(SOURCE)) instance.removeLayer(SOURCE);
        if (source) instance.removeSource(SOURCE);
        appliedTileUrl = undefined;
        appliedPaletteId = undefined;
        return;
      }
      const absoluteUrl = `${window.location.origin}${tileUrl}`;
      if (!source) {
        instance.addSource(SOURCE, {
          type: "raster",
          tiles: [absoluteUrl],
          tileSize: 512,
          minzoom: 0,
          maxzoom: 18,
        });
      } else if (appliedTileUrl !== tileUrl) {
        source.setTiles([absoluteUrl]);
      }
      appliedTileUrl = tileUrl;
      const palettePaint = heatmapPalettePaint(heatmapPalette(paletteId));
      if (!instance.getLayer(SOURCE)) {
        const labels = instance
          .getStyle()
          .layers?.find((layer) => layer.type === "symbol")?.id;
        instance.addLayer(
          {
            id: SOURCE,
            type: "raster",
            source: SOURCE,
            paint: {
              "raster-opacity": 1,
              "raster-fade-duration": 300,
              "raster-hue-rotate-transition": { duration: 0 },
              "raster-contrast-transition": { duration: 0 },
              "raster-brightness-max-transition": { duration: 0 },
              ...palettePaint,
            },
          },
          labels,
        );
      } else if (appliedPaletteId !== paletteId) {
        for (const property of Object.keys(palettePaint) as Array<
          keyof typeof palettePaint
        >) {
          instance.setPaintProperty(SOURCE, property, palettePaint[property]);
        }
      }
      appliedPaletteId = paletteId;
    };
    syncOverlay.current = overlay;
    const zonesOverlay = () => {
      if (!styleReady.current) return;
      const source = instance.getSource(ZONES_SOURCE) as
        GeoJSONSource | undefined;
      const data = zoneFeatures(current.current.zones);
      if (source) {
        source.setData(data);
      } else {
        instance.addSource(ZONES_SOURCE, {
          type: "geojson",
          data,
          cluster: true,
          clusterRadius: 64,
          clusterMaxZoom: 6,
        });
      }
      if (!instance.getLayer(ZONES_CIRCLES)) {
        instance.addLayer({
          id: ZONES_CIRCLES,
          type: "circle",
          source: ZONES_SOURCE,
          maxzoom: 7,
          filter: ["has", "point_count"],
          paint: {
            "circle-color": [
              "step",
              ["get", "point_count"],
              "#1265cc",
              10,
              "#0d4fa4",
              50,
              "#083b83",
            ],
            "circle-radius": [
              "step",
              ["get", "point_count"],
              23,
              10,
              29,
              50,
              35,
            ],
            "circle-opacity": 0.94,
            "circle-stroke-color": "#ffffff",
            "circle-stroke-width": 2,
          },
        });
        instance.addLayer({
          id: ZONES_COUNTS,
          type: "symbol",
          source: ZONES_SOURCE,
          maxzoom: 7,
          filter: ["has", "point_count"],
          layout: {
            "text-field": ["get", "point_count_abbreviated"],
            "text-font": ["Noto Sans Bold"],
            "text-size": 14,
            "text-allow-overlap": true,
          },
          paint: {
            "text-color": "#ffffff",
            "text-halo-color": "#083b83",
            "text-halo-width": 0.5,
          },
        });
        instance.addLayer({
          id: SINGLE_ZONE_CIRCLES,
          type: "circle",
          source: ZONES_SOURCE,
          maxzoom: 6,
          filter: ["!", ["has", "point_count"]],
          paint: {
            "circle-color": "#1265cc",
            "circle-radius": 20,
            "circle-opacity": 0.94,
            "circle-stroke-color": "#ffffff",
            "circle-stroke-width": 2,
          },
        });
        instance.addLayer({
          id: SINGLE_ZONE_COUNTS,
          type: "symbol",
          source: ZONES_SOURCE,
          maxzoom: 6,
          filter: ["!", ["has", "point_count"]],
          layout: {
            "text-field": "1",
            "text-font": ["Noto Sans Bold"],
            "text-size": 14,
            "text-allow-overlap": true,
          },
          paint: {
            "text-color": "#ffffff",
            "text-halo-color": "#083b83",
            "text-halo-width": 0.5,
          },
        });
      }
      if (!zoneInteractionBound.current) {
        for (const layer of [ZONES_CIRCLES, SINGLE_ZONE_CIRCLES]) {
          instance.on("mouseenter", layer, () => {
            instance.getCanvas().style.cursor = "pointer";
          });
          instance.on("mouseleave", layer, () => {
            instance.getCanvas().style.cursor = "";
          });
        }
        zoneInteractionBound.current = true;
      }
    };
    syncZones.current = zonesOverlay;
    instance.on("style.load", () => {
      // isStyleLoaded() also checks outstanding tiles. A zoom does not emit
      // style.load again, so track the style lifecycle independently.
      styleReady.current = true;
      appliedTileUrl = undefined;
      overlay();
      zonesOverlay();
    });
    instance.on("error", (event) => {
      const error = event.error as Error & { status?: number; url?: string };
      if (error.status === 409) current.current.onStale();
      else if (error.url?.includes("/heatmap-tiles/"))
        current.current.onTileError(
          error.status === 503
            ? "The map is busy. Try again shortly."
            : "Some heatmap tiles could not load. Pan the map to try again.",
        );
    });
    instance.on("movestart", () => current.current.onMapMoveStart());
    const observer = new ResizeObserver(() => instance.resize());
    observer.observe(container.current!);
    return () => {
      observer.disconnect();
      instance.remove();
      map.current = null;
      styleReady.current = false;
      syncOverlay.current = () => {};
      syncZones.current = () => {};
    };
  }, []);

  useHeatmapCamera(map, view, { metadata, zones });

  useEffect(() => {
    const instance = map.current;
    if (!instance || appliedTheme.current === theme) return;
    appliedTheme.current = theme;
    styleReady.current = false;
    instance.setStyle(styleUrl(theme));
  }, [theme]);

  useEffect(() => {
    syncOverlay.current();
  }, [metadata, tileUrl, paletteId]);

  useEffect(() => {
    syncZones.current();
  }, [zones]);

  return (
    <div className="heatmap-map absolute inset-0">
      <div
        ref={container}
        className="h-full w-full"
        role="region"
        aria-label="Personal activity heatmap"
      />
    </div>
  );
}
