"use client";

import type {
  Feature,
  FeatureCollection,
  LineString,
  Point,
  Position,
} from "geojson";
import maplibregl, {
  type GeoJSONSource,
  type StyleSpecification,
} from "maplibre-gl";
import { useEffect, useMemo, useRef, useState } from "react";
import { config } from "../lib/config";
import { type ActivityRoutePoint } from "../lib/queries";
import {
  type RouteMapBasemap,
  type RouteMapProps,
  type RouteMovingMarker,
  type RouteOverlay,
} from "./RouteMapTypes";

const DEFAULT_TOPO_STYLE_ID = "opentopomap";
const DEFAULT_STREET_STYLE_ID = "street";
const DEFAULT_SATELLITE_STYLE_ID = "satellite";
const OPEN_FREE_MAP_LIBERTY_STYLE_URL =
  "https://tiles.openfreemap.org/styles/liberty";
const OPEN_FREE_MAP_POSITRON_STYLE_URL =
  "https://tiles.openfreemap.org/styles/positron";
const CYCLING_TRAILS_SOURCE_ID = "cycling-trails";
const CYCLING_TRAILS_LAYER_ID = "cycling-trails-overlay";
const DEFAULT_BASEMAP_OPTIONS: RouteMapBasemap[] = [
  "topo",
  "street",
  "satellite",
];
const BASEMAP_LABELS: Record<RouteMapBasemap, string> = {
  topo: "Topo",
  street: "Street",
  satellite: "Satellite",
};

const EMPTY_STYLE: StyleSpecification = {
  version: 8,
  sources: {},
  layers: [
    {
      id: "background",
      type: "background",
      paint: {
        "background-color": "#f4efe5",
      },
    },
  ],
};

function buildLayeredRasterStyle({
  backgroundColor,
  baseId,
  tiles,
  attribution,
  maxZoom = 18,
  basePaint,
}: {
  backgroundColor: string;
  baseId: string;
  tiles: string[];
  attribution: string;
  maxZoom?: number;
  basePaint?: Record<string, number>;
}): StyleSpecification {
  return {
    version: 8,
    sources: {
      [baseId]: {
        type: "raster",
        tiles,
        tileSize: 256,
        attribution,
      },
    },
    layers: [
      {
        id: "background",
        type: "background",
        paint: {
          "background-color": backgroundColor,
        },
      },
      {
        id: `${baseId}-base`,
        type: "raster",
        source: baseId,
        minzoom: 0,
        maxzoom: maxZoom,
        paint: basePaint,
      },
    ],
  };
}

function buildSatelliteStyle(): StyleSpecification {
  return buildLayeredRasterStyle({
    backgroundColor: "#d8ddd8",
    baseId: "satellite",
    tiles: [
      "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
    ],
    attribution: 'Imagery &copy; <a href="https://www.esri.com/">Esri</a>',
    maxZoom: 19,
    basePaint: {
      "raster-saturation": -0.06,
      "raster-contrast": 0.05,
      "raster-brightness-min": 0.16,
      "raster-brightness-max": 0.94,
    },
  });
}

function buildBasemapStyle(
  basemap: RouteMapBasemap,
): string | StyleSpecification {
  switch (basemap) {
    case "street":
      return OPEN_FREE_MAP_POSITRON_STYLE_URL;
    case "satellite":
      return buildSatelliteStyle();
    case "topo":
    default:
      return OPEN_FREE_MAP_LIBERTY_STYLE_URL;
  }
}

function resolveConfiguredBasemap(
  styleUrl: string,
  fallback: RouteMapBasemap,
): RouteMapBasemap {
  const normalized = styleUrl.trim().toLowerCase();

  switch (normalized) {
    case "":
    case "topo":
    case DEFAULT_TOPO_STYLE_ID:
    case OPEN_FREE_MAP_LIBERTY_STYLE_URL:
      return "topo";
    case DEFAULT_STREET_STYLE_ID:
    case "streets":
    case OPEN_FREE_MAP_POSITRON_STYLE_URL:
      return "street";
    case DEFAULT_SATELLITE_STYLE_ID:
    case "imagery":
      return "satellite";
    default:
      return fallback;
  }
}

function resolveCustomMapStyle(styleUrl: string): string | null {
  const normalized = styleUrl.trim().toLowerCase();

  if (
    !normalized ||
    normalized === "topo" ||
    normalized === DEFAULT_TOPO_STYLE_ID ||
    normalized === OPEN_FREE_MAP_LIBERTY_STYLE_URL ||
    normalized === DEFAULT_STREET_STYLE_ID ||
    normalized === "streets" ||
    normalized === OPEN_FREE_MAP_POSITRON_STYLE_URL ||
    normalized === DEFAULT_SATELLITE_STYLE_ID ||
    normalized === "imagery"
  ) {
    return null;
  }

  return styleUrl;
}

const ROUTE_SOURCE_ID = "activity-route";
const ROUTE_CASING_LAYER_ID = "activity-route-casing";
const ROUTE_LAYER_ID = "activity-route-line";
const ROUTE_ARROW_LAYER_ID = "activity-route-arrows";
const ENDPOINT_SOURCE_ID = "activity-route-endpoints";
const START_LAYER_ID = "activity-route-start";
const END_LAYER_ID = "activity-route-end";
const OVERLAY_SOURCE_ID = "activity-route-overlays";
const OVERLAY_LAYER_ID = "activity-route-overlays-line";
const MARKER_SOURCE_ID = "activity-route-markers";
const MARKER_LAYER_ID = "activity-route-markers-circle";
const DEFAULT_FIT_BOUNDS_PADDING = 56;
const DEFAULT_FIT_BOUNDS_MAX_ZOOM = 14;
const EMPTY_OVERLAYS: RouteOverlay[] = [];
const EMPTY_MOVING_MARKERS: RouteMovingMarker[] = [];

type OverlayLayerEvent = {
  features?: Array<{
    properties?: {
      overlayId?: string;
    };
  }>;
};

function toPositions(points: ActivityRoutePoint[]): Position[] {
  return points.map((point) => [point.longitude, point.latitude]);
}

function buildLineFeature(
  id: string,
  points: ActivityRoutePoint[],
  properties: Record<string, number | string> = {},
): Feature<LineString> {
  return {
    type: "Feature",
    id,
    properties,
    geometry: {
      type: "LineString",
      coordinates: toPositions(points),
    },
  };
}

function buildEndpointFeature(
  id: string,
  point: ActivityRoutePoint,
  kind: "start" | "end",
): Feature<Point> {
  return {
    type: "Feature",
    id,
    properties: { kind },
    geometry: {
      type: "Point",
      coordinates: [point.longitude, point.latitude],
    },
  };
}

function buildMarkerFeature(
  id: string,
  point: ActivityRoutePoint,
  color: string,
  opacity: number,
): Feature<Point> {
  return {
    type: "Feature",
    id,
    properties: { color, opacity },
    geometry: {
      type: "Point",
      coordinates: [point.longitude, point.latitude],
    },
  };
}

function emptyFeatureCollection<
  T extends LineString | Point,
>(): FeatureCollection<T> {
  return {
    type: "FeatureCollection",
    features: [],
  };
}

function fitMapToGeometry(
  map: maplibregl.Map,
  routePoints: ActivityRoutePoint[],
  overlayPoints: ActivityRoutePoint[][],
  fitBoundsPadding: number,
  fitBoundsMaxZoom: number,
) {
  const allPoints = routePoints.concat(...overlayPoints);
  if (allPoints.length < 2) {
    return;
  }

  const bounds = allPoints.reduce(
    (currentBounds, point) =>
      currentBounds.extend([point.longitude, point.latitude]),
    new maplibregl.LngLatBounds(
      [allPoints[0].longitude, allPoints[0].latitude],
      [allPoints[0].longitude, allPoints[0].latitude],
    ),
  );

  map.fitBounds(bounds, {
    padding: fitBoundsPadding,
    maxZoom: fitBoundsMaxZoom,
    duration: 0,
  });
}

function ensureMapSourcesAndLayers(
  map: maplibregl.Map,
  showBaseTiles: boolean,
) {
  if (showBaseTiles && !map.getSource(CYCLING_TRAILS_SOURCE_ID)) {
    map.addSource(CYCLING_TRAILS_SOURCE_ID, {
      type: "raster",
      tiles: ["https://tile.waymarkedtrails.org/cycling/{z}/{x}/{y}.png"],
      tileSize: 256,
      attribution:
        'Trails: <a href="https://cycling.waymarkedtrails.org">Waymarked Trails</a>',
    });
  }

  if (showBaseTiles && !map.getLayer(CYCLING_TRAILS_LAYER_ID)) {
    const firstLabelLayerId = map
      .getStyle()
      .layers?.find((layer) => layer.type === "symbol")?.id;

    map.addLayer(
      {
        id: CYCLING_TRAILS_LAYER_ID,
        type: "raster",
        source: CYCLING_TRAILS_SOURCE_ID,
        minzoom: 0,
        maxzoom: 18,
        paint: {
          "raster-opacity": 0.86,
        },
      },
      firstLabelLayerId,
    );
  }

  if (!map.getSource(ROUTE_SOURCE_ID)) {
    map.addSource(ROUTE_SOURCE_ID, {
      type: "geojson",
      data: emptyFeatureCollection<LineString>(),
    });

    map.addLayer({
      id: ROUTE_CASING_LAYER_ID,
      type: "line",
      source: ROUTE_SOURCE_ID,
      layout: {
        "line-cap": "round",
        "line-join": "round",
      },
      paint: {
        "line-color": "#10212d",
        "line-width": 9,
        "line-opacity": 0.86,
      },
    });

    map.addLayer({
      id: ROUTE_LAYER_ID,
      type: "line",
      source: ROUTE_SOURCE_ID,
      layout: {
        "line-cap": "round",
        "line-join": "round",
      },
      paint: {
        "line-color": "#16b8a5",
        "line-width": 5,
        "line-opacity": 1,
      },
    });

    map.addLayer({
      id: ROUTE_ARROW_LAYER_ID,
      type: "symbol",
      source: ROUTE_SOURCE_ID,
      layout: {
        "symbol-placement": "line",
        "symbol-spacing": 84,
        "text-field": "▶",
        "text-size": 11,
        "text-keep-upright": false,
      },
      paint: {
        "text-color": "#e6fffb",
        "text-halo-color": "#0c5c55",
        "text-halo-width": 1.1,
        "text-opacity": 0.96,
      },
    });
  }

  if (!map.getSource(ENDPOINT_SOURCE_ID)) {
    map.addSource(ENDPOINT_SOURCE_ID, {
      type: "geojson",
      data: emptyFeatureCollection<Point>(),
    });

    map.addLayer({
      id: START_LAYER_ID,
      type: "circle",
      source: ENDPOINT_SOURCE_ID,
      filter: ["==", ["get", "kind"], "start"],
      paint: {
        "circle-radius": 7,
        "circle-color": "#0f172a",
        "circle-stroke-color": "#f8fafc",
        "circle-stroke-width": 2,
      },
    });

    map.addLayer({
      id: END_LAYER_ID,
      type: "circle",
      source: ENDPOINT_SOURCE_ID,
      filter: ["==", ["get", "kind"], "end"],
      paint: {
        "circle-radius": 7,
        "circle-color": "#f8fafc",
        "circle-stroke-color": "#0f172a",
        "circle-stroke-width": 2.5,
      },
    });
  }

  if (!map.getSource(OVERLAY_SOURCE_ID)) {
    map.addSource(OVERLAY_SOURCE_ID, {
      type: "geojson",
      data: emptyFeatureCollection<LineString>(),
    });

    map.addLayer({
      id: OVERLAY_LAYER_ID,
      type: "line",
      source: OVERLAY_SOURCE_ID,
      layout: {
        "line-cap": "round",
        "line-join": "round",
      },
      paint: {
        "line-color": ["get", "color"],
        "line-width": ["get", "weight"],
        "line-opacity": 0.96,
      },
    });
  }

  if (!map.getSource(MARKER_SOURCE_ID)) {
    map.addSource(MARKER_SOURCE_ID, {
      type: "geojson",
      data: emptyFeatureCollection<Point>(),
    });

    map.addLayer({
      id: MARKER_LAYER_ID,
      type: "circle",
      source: MARKER_SOURCE_ID,
      paint: {
        "circle-radius": 6,
        "circle-color": ["get", "color"],
        "circle-opacity": ["get", "opacity"],
        "circle-stroke-color": "#ffffff",
        "circle-stroke-width": 2,
      },
    });
  }
}

export default function MapLibreRouteMapClient({
  ariaLabel,
  routePoints,
  overlays: overlaysProp,
  movingMarkers: movingMarkersProp,
  showBaseTiles = true,
  interactive = true,
  showZoomControls = false,
  showLayerPicker = false,
  basemapOptions,
  defaultBasemap = "topo",
  fitBoundsPadding = DEFAULT_FIT_BOUNDS_PADDING,
  fitBoundsMaxZoom = DEFAULT_FIT_BOUNDS_MAX_ZOOM,
}: RouteMapProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const mapRef = useRef<maplibregl.Map | null>(null);
  const resizeFrameRef = useRef<number | null>(null);
  const appliedStyleKeyRef = useRef<string | null>(null);
  const preservedViewStateRef = useRef<{
    center: [number, number];
    zoom: number;
    bearing: number;
    pitch: number;
  } | null>(null);
  const shouldRestoreViewRef = useRef(false);
  const overlayClickHandlerRef = useRef<
    ((event: OverlayLayerEvent) => void) | null
  >(null);
  const overlayMouseEnterHandlerRef = useRef<
    ((event: OverlayLayerEvent) => void) | null
  >(null);
  const overlayMouseLeaveHandlerRef = useRef<
    ((event: OverlayLayerEvent) => void) | null
  >(null);
  const overlays = overlaysProp ?? EMPTY_OVERLAYS;
  const movingMarkers = movingMarkersProp ?? EMPTY_MOVING_MARKERS;
  const styleUrl = config.MAP_STYLE_URL;
  const availableBasemaps = useMemo(
    () =>
      basemapOptions && basemapOptions.length > 0
        ? basemapOptions
        : DEFAULT_BASEMAP_OPTIONS,
    [basemapOptions],
  );
  const configuredBasemap = useMemo(
    () => resolveConfiguredBasemap(styleUrl, defaultBasemap),
    [defaultBasemap, styleUrl],
  );
  const customMapStyle = useMemo(
    () => resolveCustomMapStyle(styleUrl),
    [styleUrl],
  );
  const canShowLayerPicker =
    interactive &&
    showBaseTiles &&
    showLayerPicker &&
    availableBasemaps.length > 1;
  const [selectedBasemap, setSelectedBasemap] =
    useState<RouteMapBasemap>(configuredBasemap);

  useEffect(() => {
    setSelectedBasemap(configuredBasemap);
  }, [configuredBasemap]);

  const mapStyle = useMemo(() => {
    if (!showBaseTiles) {
      return EMPTY_STYLE;
    }

    if (canShowLayerPicker) {
      return buildBasemapStyle(selectedBasemap);
    }

    if (customMapStyle) {
      return customMapStyle;
    }

    return buildBasemapStyle(configuredBasemap);
  }, [
    canShowLayerPicker,
    configuredBasemap,
    customMapStyle,
    selectedBasemap,
    showBaseTiles,
  ]);
  const mapStyleKey = useMemo(() => {
    if (!showBaseTiles) {
      return "empty";
    }

    if (canShowLayerPicker) {
      return `basemap:${selectedBasemap}`;
    }

    if (customMapStyle) {
      return `custom:${customMapStyle}`;
    }

    return `basemap:${configuredBasemap}`;
  }, [
    canShowLayerPicker,
    configuredBasemap,
    customMapStyle,
    selectedBasemap,
    showBaseTiles,
  ]);

  const routeSourceData = useMemo<FeatureCollection<LineString>>(
    () => ({
      type: "FeatureCollection",
      features:
        (routePoints?.length ?? 0) >= 2
          ? [buildLineFeature("route", routePoints ?? [])]
          : [],
    }),
    [routePoints],
  );

  const endpointSourceData = useMemo<FeatureCollection<Point>>(() => {
    if ((routePoints?.length ?? 0) < 2) {
      return emptyFeatureCollection<Point>();
    }

    const points = routePoints ?? [];

    return {
      type: "FeatureCollection",
      features: [
        buildEndpointFeature("start", points[0], "start"),
        buildEndpointFeature("end", points.at(-1) ?? points[0], "end"),
      ],
    };
  }, [routePoints]);

  const overlaySourceData = useMemo<FeatureCollection<LineString>>(
    () => ({
      type: "FeatureCollection",
      features: overlays
        .filter((overlay) => overlay.points.length >= 2)
        .map((overlay) =>
          buildLineFeature(overlay.id, overlay.points, {
            overlayId: overlay.id,
            color: overlay.color,
            weight: overlay.weight ?? 6,
          }),
        ),
    }),
    [overlays],
  );

  const markerSourceData = useMemo<FeatureCollection<Point>>(
    () => ({
      type: "FeatureCollection",
      features: movingMarkers
        .filter((marker) => marker.point)
        .map((marker) =>
          buildMarkerFeature(
            marker.id,
            marker.point as ActivityRoutePoint,
            marker.color,
            marker.opacity ?? 1,
          ),
        ),
    }),
    [movingMarkers],
  );

  const overlayHandlers = useMemo(
    () =>
      new Map(
        overlays
          .filter((overlay) => overlay.onClick)
          .map((overlay) => [overlay.id, overlay.onClick as () => void]),
      ),
    [overlays],
  );

  useEffect(() => {
    if (!containerRef.current || mapRef.current) {
      return undefined;
    }

    const map = new maplibregl.Map({
      container: containerRef.current,
      style: mapStyle,
      attributionControl: showBaseTiles ? { compact: true } : false,
      dragRotate: false,
      pitchWithRotate: false,
      interactive,
      touchPitch: false,
      maxPitch: 0,
    });

    appliedStyleKeyRef.current = mapStyleKey;

    if (interactive && showZoomControls) {
      map.addControl(
        new maplibregl.NavigationControl({
          showCompass: false,
          visualizePitch: false,
        }),
        "top-right",
      );
    }

    const handleResize = () => {
      if (resizeFrameRef.current != null) {
        cancelAnimationFrame(resizeFrameRef.current);
      }

      resizeFrameRef.current = requestAnimationFrame(() => {
        if (mapRef.current !== map || !containerRef.current?.isConnected) {
          return;
        }

        map.resize();
      });
    };

    mapRef.current = map;

    window.addEventListener("resize", handleResize);
    document.addEventListener("fullscreenchange", handleResize);

    resizeFrameRef.current = requestAnimationFrame(() => {
      if (mapRef.current !== map || !containerRef.current?.isConnected) {
        return;
      }

      map.resize();
    });

    return () => {
      window.removeEventListener("resize", handleResize);
      document.removeEventListener("fullscreenchange", handleResize);
      if (resizeFrameRef.current != null) {
        cancelAnimationFrame(resizeFrameRef.current);
        resizeFrameRef.current = null;
      }
      map.remove();
      mapRef.current = null;
      overlayClickHandlerRef.current = null;
      overlayMouseEnterHandlerRef.current = null;
      overlayMouseLeaveHandlerRef.current = null;
      appliedStyleKeyRef.current = null;
      preservedViewStateRef.current = null;
      shouldRestoreViewRef.current = false;
    };
  }, [interactive, mapStyle, mapStyleKey, showBaseTiles, showZoomControls]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || appliedStyleKeyRef.current === mapStyleKey) {
      return;
    }

    preservedViewStateRef.current = {
      center: [map.getCenter().lng, map.getCenter().lat],
      zoom: map.getZoom(),
      bearing: map.getBearing(),
      pitch: map.getPitch(),
    };
    shouldRestoreViewRef.current = true;
    appliedStyleKeyRef.current = mapStyleKey;
    map.setStyle(mapStyle);
  }, [mapStyle, mapStyleKey]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map) {
      return undefined;
    }

    const syncMap = () => {
      if (mapRef.current !== map) {
        return;
      }

      ensureMapSourcesAndLayers(map, showBaseTiles);

      (map.getSource(ROUTE_SOURCE_ID) as GeoJSONSource).setData(
        routeSourceData,
      );
      (map.getSource(ENDPOINT_SOURCE_ID) as GeoJSONSource).setData(
        endpointSourceData,
      );
      (map.getSource(OVERLAY_SOURCE_ID) as GeoJSONSource).setData(
        overlaySourceData,
      );

      if (overlayClickHandlerRef.current) {
        map.off("click", OVERLAY_LAYER_ID, overlayClickHandlerRef.current);
        overlayClickHandlerRef.current = null;
      }
      if (overlayMouseEnterHandlerRef.current) {
        map.off(
          "mouseenter",
          OVERLAY_LAYER_ID,
          overlayMouseEnterHandlerRef.current,
        );
        overlayMouseEnterHandlerRef.current = null;
      }
      if (overlayMouseLeaveHandlerRef.current) {
        map.off(
          "mouseleave",
          OVERLAY_LAYER_ID,
          overlayMouseLeaveHandlerRef.current,
        );
        overlayMouseLeaveHandlerRef.current = null;
      }

      if (overlayHandlers.size > 0) {
        const handleOverlayClick = (event: OverlayLayerEvent) => {
          const overlayId = event.features?.[0]?.properties?.overlayId;
          if (!overlayId) {
            return;
          }

          overlayHandlers.get(overlayId)?.();
        };
        const handleOverlayMouseEnter = (_event: OverlayLayerEvent) => {
          map.getCanvas().style.cursor = "pointer";
        };
        const handleOverlayMouseLeave = (_event: OverlayLayerEvent) => {
          map.getCanvas().style.cursor = "";
        };

        map.on("click", OVERLAY_LAYER_ID, handleOverlayClick);
        map.on("mouseenter", OVERLAY_LAYER_ID, handleOverlayMouseEnter);
        map.on("mouseleave", OVERLAY_LAYER_ID, handleOverlayMouseLeave);

        overlayClickHandlerRef.current = handleOverlayClick;
        overlayMouseEnterHandlerRef.current = handleOverlayMouseEnter;
        overlayMouseLeaveHandlerRef.current = handleOverlayMouseLeave;
      } else {
        map.getCanvas().style.cursor = "";
      }

      if (shouldRestoreViewRef.current && preservedViewStateRef.current) {
        map.jumpTo(preservedViewStateRef.current);
        shouldRestoreViewRef.current = false;
        preservedViewStateRef.current = null;
        return;
      }

      fitMapToGeometry(
        map,
        routePoints ?? [],
        overlays.map((overlay) => overlay.points),
        fitBoundsPadding,
        fitBoundsMaxZoom,
      );
    };

    if (map.isStyleLoaded()) {
      syncMap();
      return undefined;
    }

    map.once("load", syncMap);

    return () => {
      map.off("load", syncMap);
    };
  }, [
    endpointSourceData,
    overlayHandlers,
    overlaySourceData,
    overlays,
    routePoints,
    routeSourceData,
    fitBoundsMaxZoom,
    fitBoundsPadding,
    mapStyleKey,
    showBaseTiles,
  ]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map) {
      return undefined;
    }

    const syncMarkers = () => {
      if (mapRef.current !== map) {
        return;
      }

      ensureMapSourcesAndLayers(map, showBaseTiles);
      (map.getSource(MARKER_SOURCE_ID) as GeoJSONSource).setData(
        markerSourceData,
      );
    };

    if (map.isStyleLoaded()) {
      syncMarkers();
      return undefined;
    }

    map.once("load", syncMarkers);

    return () => {
      map.off("load", syncMarkers);
    };
  }, [markerSourceData, mapStyleKey, showBaseTiles]);

  return (
    <div className="relative h-full w-full">
      {canShowLayerPicker ? (
        <div className="absolute left-3 top-3 z-10 flex flex-wrap gap-2 rounded-box border border-base-300/80 bg-base-100/90 p-1 shadow-sm backdrop-blur">
          {availableBasemaps.map((basemap) => {
            const isActive = basemap === selectedBasemap;

            return (
              <button
                key={basemap}
                type="button"
                className={`btn btn-xs ${isActive ? "btn-primary" : "btn-ghost"}`}
                aria-pressed={isActive}
                onClick={() => {
                  setSelectedBasemap(basemap);
                }}
              >
                {BASEMAP_LABELS[basemap]}
              </button>
            );
          })}
        </div>
      ) : null}

      <div
        ref={containerRef}
        role="img"
        aria-label={ariaLabel}
        className="h-full w-full"
      />
    </div>
  );
}
