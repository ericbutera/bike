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
import { useEffect, useMemo, useRef } from "react";
import { config } from "../lib/config";
import { type ActivityRoutePoint } from "../lib/queries";
import { type RouteMapProps } from "./RouteMapTypes";

const DEFAULT_TOPO_STYLE_ID = "opentopomap";

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

function buildTopographicStyle(): StyleSpecification {
  return {
    version: 8,
    sources: {
      opentopomap: {
        type: "raster",
        tiles: [
          "https://a.tile.opentopomap.org/{z}/{x}/{y}.png",
          "https://b.tile.opentopomap.org/{z}/{x}/{y}.png",
          "https://c.tile.opentopomap.org/{z}/{x}/{y}.png",
        ],
        tileSize: 256,
        attribution:
          '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors, <a href="https://viewfinderpanoramas.org">SRTM</a> | Map style: <a href="https://opentopomap.org">OpenTopoMap</a>',
      },
    },
    layers: [
      {
        id: "background",
        type: "background",
        paint: {
          "background-color": "#e7ede2",
        },
      },
      {
        id: "opentopomap-base",
        type: "raster",
        source: "opentopomap",
        minzoom: 0,
        maxzoom: 17,
        paint: {
          "raster-saturation": -0.06,
          "raster-contrast": 0.08,
          "raster-brightness-min": 0.08,
          "raster-brightness-max": 0.98,
        },
      },
    ],
  };
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
const FIT_BOUNDS_PADDING = 56;

function resolveMapStyle(styleUrl: string): string | StyleSpecification {
  const normalized = styleUrl.trim().toLowerCase();

  if (!normalized || normalized === DEFAULT_TOPO_STYLE_ID) {
    return buildTopographicStyle();
  }

  return styleUrl;
}

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
    padding: FIT_BOUNDS_PADDING,
    maxZoom: 14,
    duration: 0,
  });
}

function ensureMapSourcesAndLayers(map: maplibregl.Map) {
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
  routePoints,
  overlays = [],
  movingMarkers = [],
  showBaseTiles = true,
  interactive = true,
}: RouteMapProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const mapRef = useRef<maplibregl.Map | null>(null);
  const resizeFrameRef = useRef<number | null>(null);
  const overlayClickHandlerRef = useRef<
    ((event: OverlayLayerEvent) => void) | null
  >(null);
  const overlayMouseEnterHandlerRef = useRef<
    ((event: OverlayLayerEvent) => void) | null
  >(null);
  const overlayMouseLeaveHandlerRef = useRef<
    ((event: OverlayLayerEvent) => void) | null
  >(null);
  const styleUrl = config.MAP_STYLE_URL;
  const mapStyle = useMemo(
    () => (showBaseTiles ? resolveMapStyle(styleUrl) : EMPTY_STYLE),
    [showBaseTiles, styleUrl],
  );

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
    };
  }, [interactive, mapStyle, showBaseTiles]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map) {
      return undefined;
    }

    const syncMap = () => {
      if (mapRef.current !== map) {
        return;
      }

      ensureMapSourcesAndLayers(map);

      (map.getSource(ROUTE_SOURCE_ID) as GeoJSONSource).setData(
        routeSourceData,
      );
      (map.getSource(ENDPOINT_SOURCE_ID) as GeoJSONSource).setData(
        endpointSourceData,
      );
      (map.getSource(OVERLAY_SOURCE_ID) as GeoJSONSource).setData(
        overlaySourceData,
      );
      (map.getSource(MARKER_SOURCE_ID) as GeoJSONSource).setData(
        markerSourceData,
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

      fitMapToGeometry(
        map,
        routePoints ?? [],
        overlays.map((overlay) => overlay.points),
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
    markerSourceData,
    overlayHandlers,
    overlaySourceData,
    overlays,
    routePoints,
    routeSourceData,
  ]);

  return <div ref={containerRef} className="h-full w-full" />;
}
