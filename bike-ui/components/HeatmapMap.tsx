"use client";

import * as maplibregl from "../lib/maplibre";
import {
  type GeoJSONSource,
  type MapLayerMouseEvent,
  type RasterTileSource,
} from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { useEffect, useRef } from "react";
import type { FeatureCollection, Point } from "geojson";
import { useBikeTheme } from "../lib/useBikeTheme";
import type { HeatmapMetadata, HeatmapZones } from "../lib/heatmaps";
import {
  heatmapPalette,
  heatmapPalettePaint,
  type HeatmapPaletteId,
} from "../lib/heatmapColors";

const SOURCE = "personal-heatmap";
const ZONES_SOURCE = "personal-heatmap-zones";
const ZONES_CIRCLES = "personal-heatmap-zone-circles";
const ZONES_COUNTS = "personal-heatmap-zone-counts";
const SINGLE_ZONE_CIRCLES = "personal-heatmap-single-zone-circles";
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
  locationRequest,
  onLocationError,
  onMapMoveStart,
}: {
  metadata?: HeatmapMetadata;
  tileUrl?: string;
  zones: HeatmapZones["zones"];
  paletteId?: HeatmapPaletteId;
  onStale: () => void;
  onTileError: (message: string) => void;
  locationRequest: number;
  onLocationError: (message: string) => void;
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
    onLocationError,
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
      onLocationError,
      onMapMoveStart,
    };
  }, [
    metadata,
    tileUrl,
    zones,
    paletteId,
    onStale,
    onTileError,
    onLocationError,
    onMapMoveStart,
  ]);
  const fitted = useRef(false);
  const zoneInteractionBound = useRef(false);
  const styleReady = useRef(false);
  const syncOverlay = useRef(() => {});
  const syncZones = useRef(() => {});
  const theme = useBikeTheme();
  const appliedTheme = useRef(theme);

  useEffect(() => {
    const instance = new maplibregl.Map({
      container: container.current!,
      style: styleUrl(appliedTheme.current),
      center: [-85.6, 44.7],
      zoom: 6,
      minZoom: 0,
      maxZoom: 18,
      attributionControl: false,
    });
    map.current = instance;
    instance.addControl(new maplibregl.NavigationControl(), "bottom-right");
    instance.addControl(
      new maplibregl.AttributionControl({ compact: false }),
      "bottom-left",
    );
    let appliedTileUrl: string | undefined;
    let appliedPaletteId: HeatmapPaletteId | undefined;
    const overlay = () => {
      if (!styleReady.current) return;
      const { tileUrl, metadata, paletteId } = current.current;
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
      if (!fitted.current && metadata?.bounds) {
        const [west, south, east, north] = metadata.bounds;
        instance.fitBounds(
          [
            [west, south],
            [east, north],
          ],
          { padding: 50, maxZoom: 14, duration: 0 },
        );
        fitted.current = true;
      }
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
        const zoomToZone = (event: MapLayerMouseEvent) => {
          const feature = event.features?.[0];
          if (!feature || feature.geometry.type !== "Point") return;
          const center = feature.geometry.coordinates as [number, number];
          const clusterId = feature.properties?.cluster_id;
          fitted.current = true;
          if (typeof clusterId === "number") {
            const source = instance.getSource(ZONES_SOURCE) as
              GeoJSONSource | undefined;
            if (source) {
              void source
                .getClusterExpansionZoom(clusterId)
                .then((expansionZoom) => {
                  if (map.current !== instance) return;
                  instance.easeTo({
                    center,
                    zoom: Math.max(expansionZoom, instance.getZoom() + 2),
                    duration: 600,
                  });
                })
                .catch(() => {
                  // The zone source can refresh while expansion is resolving.
                });
            }
          } else {
            instance.easeTo({
              center,
              zoom: Math.min(18, Math.max(instance.getZoom() + 2, 8)),
              duration: 600,
            });
          }
        };
        for (const layer of [ZONES_CIRCLES, SINGLE_ZONE_CIRCLES]) {
          instance.on("click", layer, zoomToZone);
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

  useEffect(() => {
    if (locationRequest === 0) return;
    const instance = map.current;
    if (!instance) return;
    if (!navigator.geolocation) {
      current.current.onLocationError(
        "Location is not available in this browser.",
      );
      return;
    }
    let cancelled = false;
    navigator.geolocation.getCurrentPosition(
      ({ coords }) => {
        if (cancelled || map.current !== instance) return;
        fitted.current = true;
        instance.flyTo({
          center: [coords.longitude, coords.latitude],
          zoom: 12,
          duration: 900,
        });
      },
      (error) => {
        if (cancelled || map.current !== instance) return;
        current.current.onLocationError(
          error.code === error.PERMISSION_DENIED
            ? "Location access was denied. Allow location access in your browser and try again."
            : "Could not determine your location. Try again.",
        );
      },
      { enableHighAccuracy: false, maximumAge: 300_000, timeout: 10_000 },
    );
    return () => {
      cancelled = true;
    };
  }, [locationRequest]);

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
