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
  type RouteMapFollowViewportBehavior,
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
const MARKER_LABEL_LAYER_ID = "activity-route-markers-label";
const DEFAULT_FIT_BOUNDS_PADDING = 56;
const DEFAULT_FIT_BOUNDS_MAX_ZOOM = 14;
const FOLLOW_VIEWPORT_EASE_DURATION_MS = 1400;
const FOLLOW_VIEWPORT_CENTER_SMOOTHING_ALPHA = 0.16;
const FOLLOW_VIEWPORT_ZOOM_SMOOTHING_ALPHA = 0.04;
const DEFAULT_MOVING_MARKER_TRANSITION_MS = 0;
const MIN_MARKER_ANIMATION_FRAME_MS = 1000 / 30;
const EMPTY_OVERLAYS: RouteOverlay[] = [];
const EMPTY_MOVING_MARKERS: RouteMovingMarker[] = [];

type FollowCameraTarget = {
  center: [number, number];
  zoom: number;
};

type OverlayLayerEvent = {
  point?: {
    x: number;
    y: number;
  };
  features?: Array<{
    properties?: {
      overlayId?: string;
      label?: string;
    };
  }>;
};

type MapInteractionEvent = {
  originalEvent?: unknown;
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
  label: string | null,
): Feature<Point> {
  return {
    type: "Feature",
    id,
    properties: { color, opacity, label },
    geometry: {
      type: "Point",
      coordinates: [point.longitude, point.latitude],
    },
  };
}

function interpolateOptionalNumber(
  previousValue: number | null | undefined,
  currentValue: number | null | undefined,
  progress: number,
) {
  if (previousValue == null && currentValue == null) {
    return null;
  }

  if (previousValue == null) {
    return currentValue ?? null;
  }

  if (currentValue == null) {
    return previousValue;
  }

  return previousValue + (currentValue - previousValue) * progress;
}

function interpolateNumber(
  previousValue: number,
  currentValue: number,
  smoothingAlpha: number,
) {
  return previousValue + (currentValue - previousValue) * smoothingAlpha;
}

function smoothFollowCameraTarget(
  previousTarget: FollowCameraTarget | null,
  nextTarget: FollowCameraTarget,
): FollowCameraTarget {
  if (!previousTarget) {
    return nextTarget;
  }

  return {
    center: [
      interpolateNumber(
        previousTarget.center[0],
        nextTarget.center[0],
        FOLLOW_VIEWPORT_CENTER_SMOOTHING_ALPHA,
      ),
      interpolateNumber(
        previousTarget.center[1],
        nextTarget.center[1],
        FOLLOW_VIEWPORT_CENTER_SMOOTHING_ALPHA,
      ),
    ],
    zoom: interpolateNumber(
      previousTarget.zoom,
      nextTarget.zoom,
      FOLLOW_VIEWPORT_ZOOM_SMOOTHING_ALPHA,
    ),
  };
}

function resolveFollowViewportZoom({
  requestedZoom,
  preserveUserZoom,
  userFollowZoom,
}: {
  requestedZoom: number;
  preserveUserZoom: boolean;
  userFollowZoom: { zoom: number } | null;
}) {
  if (!preserveUserZoom || !userFollowZoom) {
    return requestedZoom;
  }

  return userFollowZoom.zoom;
}

function distanceRange(points: ActivityRoutePoint[] | null | undefined) {
  if (!points || points.length < 2) {
    return null;
  }

  const firstDistance = points[0]?.distance_meters;
  const lastDistance = points.at(-1)?.distance_meters;
  const hasDistanceRange =
    typeof firstDistance === "number" &&
    typeof lastDistance === "number" &&
    lastDistance > firstDistance &&
    points.every((point) => typeof point.distance_meters === "number");

  if (!hasDistanceRange) {
    return null;
  }

  return {
    firstDistance,
    lastDistance,
    totalDistance: lastDistance - firstDistance,
  };
}

function clampProgress(value: number) {
  return Math.min(Math.max(value, 0), 1);
}

function interpolateRoutePointByProgress(
  points: ActivityRoutePoint[] | null | undefined,
  progress: number,
) {
  if (!points || points.length === 0) {
    return null;
  }

  if (points.length === 1) {
    return points[0];
  }

  const clampedProgress = clampProgress(progress);

  if (clampedProgress <= 0) {
    return points[0];
  }

  const range = distanceRange(points);
  const firstDistance = range?.firstDistance;
  const lastDistance = range?.lastDistance;
  const hasDistanceRange = Boolean(range);
  const targetMeasure = hasDistanceRange
    ? (firstDistance as number) +
      clampedProgress * ((lastDistance as number) - (firstDistance as number))
    : clampedProgress * (points.length - 1);

  for (let index = 1; index < points.length; index += 1) {
    const previous = points[index - 1];
    const current = points[index];
    const previousMeasure = hasDistanceRange
      ? (previous.distance_meters as number)
      : index - 1;
    const currentMeasure = hasDistanceRange
      ? (current.distance_meters as number)
      : index;

    if (targetMeasure <= currentMeasure) {
      const span = Math.max(currentMeasure - previousMeasure, Number.EPSILON);
      const localProgress = (targetMeasure - previousMeasure) / span;

      return {
        ...current,
        elapsed_seconds:
          previous.elapsed_seconds +
          (current.elapsed_seconds - previous.elapsed_seconds) * localProgress,
        latitude:
          previous.latitude +
          (current.latitude - previous.latitude) * localProgress,
        longitude:
          previous.longitude +
          (current.longitude - previous.longitude) * localProgress,
        distance_meters: hasDistanceRange
          ? targetMeasure - (firstDistance as number)
          : interpolateOptionalNumber(
              previous.distance_meters,
              current.distance_meters,
              localProgress,
            ),
        elevation_meters: interpolateOptionalNumber(
          previous.elevation_meters,
          current.elevation_meters,
          localProgress,
        ),
        speed_mps: interpolateOptionalNumber(
          previous.speed_mps,
          current.speed_mps,
          localProgress,
        ),
        heart_rate_bpm: interpolateOptionalNumber(
          previous.heart_rate_bpm,
          current.heart_rate_bpm,
          localProgress,
        ),
        cadence_rpm: interpolateOptionalNumber(
          previous.cadence_rpm,
          current.cadence_rpm,
          localProgress,
        ),
        power_watts: interpolateOptionalNumber(
          previous.power_watts,
          current.power_watts,
          localProgress,
        ),
      };
    }
  }

  return points.at(-1) ?? null;
}

function buildMarkerSourceData(
  movingMarkers: RouteMovingMarker[],
): FeatureCollection<Point> {
  return {
    type: "FeatureCollection",
    features: movingMarkers
      .filter((marker) => marker.point)
      .map((marker) =>
        buildMarkerFeature(
          marker.id,
          marker.point as ActivityRoutePoint,
          marker.color,
          marker.opacity ?? 1,
          marker.label ?? null,
        ),
      ),
  };
}

function interpolateMovingMarker(
  routePoints: ActivityRoutePoint[] | null | undefined,
  previousMarker: RouteMovingMarker,
  nextMarker: RouteMovingMarker,
  animationProgress: number,
): RouteMovingMarker {
  if (
    !previousMarker.point ||
    !nextMarker.point ||
    typeof previousMarker.progress !== "number" ||
    typeof nextMarker.progress !== "number"
  ) {
    return nextMarker;
  }

  const interpolatedProgress =
    previousMarker.progress +
    (nextMarker.progress - previousMarker.progress) * animationProgress;
  const interpolatedPoint = interpolateRoutePointByProgress(
    routePoints,
    interpolatedProgress,
  );

  if (!interpolatedPoint) {
    return nextMarker;
  }

  return {
    ...nextMarker,
    progress: interpolatedProgress,
    point: interpolatedPoint,
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
        "circle-radius": 10,
        "circle-color": ["get", "color"],
        "circle-opacity": ["get", "opacity"],
        "circle-stroke-color": "#ffffff",
        "circle-stroke-width": 2.5,
      },
    });

    map.addLayer({
      id: MARKER_LABEL_LAYER_ID,
      type: "symbol",
      source: MARKER_SOURCE_ID,
      layout: {
        "text-field": ["coalesce", ["get", "label"], ""],
        "text-size": 11,
        "text-allow-overlap": true,
        "text-ignore-placement": true,
      },
      paint: {
        "text-color": "#ffffff",
        "text-opacity": ["get", "opacity"],
        "text-halo-color": "#0f172a",
        "text-halo-width": 1.2,
      },
    });
  }
}

export default function MapLibreRouteMapClient({
  ariaLabel,
  routePoints,
  overlays: overlaysProp,
  movingMarkers: movingMarkersProp,
  movingMarkerTransitionMs = DEFAULT_MOVING_MARKER_TRANSITION_MS,
  followViewport = null,
  followViewportBehavior = "ease",
  followViewportPreserveUserZoom = false,
  layerPickerClassName,
  showBaseTiles = true,
  interactive = true,
  showZoomControls = false,
  showLayerPicker = false,
  basemapOptions,
  defaultBasemap = "topo",
  selectedBasemap: selectedBasemapProp,
  onSelectedBasemapChange,
  fitBoundsPoints,
  fitBoundsKey,
  fitBoundsPadding = DEFAULT_FIT_BOUNDS_PADDING,
  fitBoundsMaxZoom = DEFAULT_FIT_BOUNDS_MAX_ZOOM,
  showRouteEndpoints = true,
}: RouteMapProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const mapRef = useRef<maplibregl.Map | null>(null);
  const resizeFrameRef = useRef<number | null>(null);
  const markerAnimationFrameRef = useRef<number | null>(null);
  const lastMovingMarkerUpdateAtRef = useRef<number | null>(null);
  const renderedMovingMarkersRef = useRef<RouteMovingMarker[]>([]);
  const lastFollowViewportKeyRef = useRef<string | null>(null);
  const smoothedFollowTargetRef = useRef<FollowCameraTarget | null>(null);
  const isUserChangingZoomRef = useRef(false);
  const userFollowZoomRef = useRef<{
    zoom: number;
  } | null>(null);
  const lastFittedRoutePointsRef = useRef<
    ActivityRoutePoint[] | null | undefined
  >(null);
  const lastFittedFitBoundsPointsRef = useRef<
    ActivityRoutePoint[] | null | undefined
  >(null);
  const lastFitBoundsKeyRef = useRef<string | number | null | undefined>(
    undefined,
  );
  const hasFittedInitialViewRef = useRef(false);
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
  const overlayMouseMoveHandlerRef = useRef<
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
  const [uncontrolledSelectedBasemap, setUncontrolledSelectedBasemap] =
    useState<RouteMapBasemap>(configuredBasemap);
  const isBasemapControlled = selectedBasemapProp != null;
  const selectedBasemap = selectedBasemapProp ?? uncontrolledSelectedBasemap;
  const [overlayTooltip, setOverlayTooltip] = useState<{
    label: string;
    x: number;
    y: number;
  } | null>(null);

  useEffect(() => {
    if (!isBasemapControlled) {
      setUncontrolledSelectedBasemap(configuredBasemap);
    }
  }, [configuredBasemap, isBasemapControlled]);

  const handleBasemapChange = (basemap: RouteMapBasemap) => {
    if (!isBasemapControlled) {
      setUncontrolledSelectedBasemap(basemap);
    }

    onSelectedBasemapChange?.(basemap);
  };

  const mapStyle = useMemo(() => {
    if (!showBaseTiles) {
      return EMPTY_STYLE;
    }

    // If the basemap is controlled by the parent component, prefer it
    // even if the internal layer picker UI is not shown. This allows
    // external controls (like a top-level join) to change the map style.
    if (isBasemapControlled) {
      return buildBasemapStyle(selectedBasemap);
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
    isBasemapControlled,
  ]);
  const mapStyleKey = useMemo(() => {
    if (!showBaseTiles) {
      return "empty";
    }

    if (isBasemapControlled) {
      return `basemap:${selectedBasemap}`;
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
    isBasemapControlled,
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
    if (!showRouteEndpoints || (routePoints?.length ?? 0) < 2) {
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
  }, [routePoints, showRouteEndpoints]);

  const overlaySourceData = useMemo<FeatureCollection<LineString>>(
    () => ({
      type: "FeatureCollection",
      features: overlays
        .filter((overlay) => overlay.points.length >= 2)
        .map((overlay) =>
          buildLineFeature(overlay.id, overlay.points, {
            overlayId: overlay.id,
            color: overlay.color,
            label: overlay.label ?? "",
            weight: overlay.weight ?? 6,
          }),
        ),
    }),
    [overlays],
  );

  const markerSourceData = useMemo<FeatureCollection<Point>>(
    () => buildMarkerSourceData(movingMarkers),
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
      attributionControl: false,
      dragRotate: false,
      pitchWithRotate: false,
      interactive,
      touchPitch: false,
      maxPitch: 0,
    });

    appliedStyleKeyRef.current = mapStyleKey;

    if (showBaseTiles) {
      map.addControl(
        new maplibregl.AttributionControl({ compact: true }),
        "bottom-right",
      );
      requestAnimationFrame(() => {
        containerRef.current
          ?.querySelector(".maplibregl-ctrl-attrib")
          ?.classList.remove("maplibregl-compact-show");
      });
    }

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
      if (markerAnimationFrameRef.current != null) {
        cancelAnimationFrame(markerAnimationFrameRef.current);
        markerAnimationFrameRef.current = null;
      }
      map.remove();
      mapRef.current = null;
      overlayClickHandlerRef.current = null;
      overlayMouseEnterHandlerRef.current = null;
      overlayMouseMoveHandlerRef.current = null;
      overlayMouseLeaveHandlerRef.current = null;
      appliedStyleKeyRef.current = null;
      preservedViewStateRef.current = null;
      lastMovingMarkerUpdateAtRef.current = null;
      renderedMovingMarkersRef.current = [];
      shouldRestoreViewRef.current = false;
      lastFollowViewportKeyRef.current = null;
      smoothedFollowTargetRef.current = null;
      isUserChangingZoomRef.current = false;
      userFollowZoomRef.current = null;
      lastFittedRoutePointsRef.current = null;
      lastFittedFitBoundsPointsRef.current = null;
      lastFitBoundsKeyRef.current = undefined;
      hasFittedInitialViewRef.current = false;
    };
  }, [interactive, mapStyle, mapStyleKey, showBaseTiles, showZoomControls]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !followViewportPreserveUserZoom) {
      userFollowZoomRef.current = null;
      isUserChangingZoomRef.current = false;
      return undefined;
    }

    const handleZoomStart = (event: MapInteractionEvent) => {
      if (!event.originalEvent || mapRef.current !== map) {
        return;
      }

      isUserChangingZoomRef.current = true;
    };
    const handleZoomEnd = (event: MapInteractionEvent) => {
      if (!event.originalEvent || mapRef.current !== map) {
        return;
      }

      userFollowZoomRef.current = {
        zoom: map.getZoom(),
      };
      isUserChangingZoomRef.current = false;
      smoothedFollowTargetRef.current = {
        center: [map.getCenter().lng, map.getCenter().lat],
        zoom: map.getZoom(),
      };
    };

    map.on("zoomstart", handleZoomStart);
    map.on("zoomend", handleZoomEnd);

    return () => {
      map.off("zoomstart", handleZoomStart);
      map.off("zoomend", handleZoomEnd);
      isUserChangingZoomRef.current = false;
    };
  }, [followViewportPreserveUserZoom]);

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

      const shouldFitToGeometry =
        !hasFittedInitialViewRef.current ||
        lastFittedRoutePointsRef.current !== routePoints ||
        lastFittedFitBoundsPointsRef.current !== fitBoundsPoints ||
        lastFitBoundsKeyRef.current !== fitBoundsKey;

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
      if (overlayMouseMoveHandlerRef.current) {
        map.off(
          "mousemove",
          OVERLAY_LAYER_ID,
          overlayMouseMoveHandlerRef.current,
        );
        overlayMouseMoveHandlerRef.current = null;
      }
      if (overlayMouseLeaveHandlerRef.current) {
        map.off(
          "mouseleave",
          OVERLAY_LAYER_ID,
          overlayMouseLeaveHandlerRef.current,
        );
        overlayMouseLeaveHandlerRef.current = null;
      }

      if (overlays.length > 0) {
        const handleOverlayClick = (event: OverlayLayerEvent) => {
          const overlayId = event.features?.[0]?.properties?.overlayId;
          if (!overlayId) {
            return;
          }

          overlayHandlers.get(overlayId)?.();
        };
        const handleOverlayMouseEnter = (_event: OverlayLayerEvent) => {
          map.getCanvas().style.cursor =
            overlayHandlers.size > 0 ? "pointer" : "";
        };
        const handleOverlayMouseMove = (event: OverlayLayerEvent) => {
          const label = event.features?.[0]?.properties?.label;

          if (!label || !event.point) {
            setOverlayTooltip(null);
            return;
          }

          setOverlayTooltip({
            label,
            x: event.point.x,
            y: event.point.y,
          });
        };
        const handleOverlayMouseLeave = (_event: OverlayLayerEvent) => {
          map.getCanvas().style.cursor = "";
          setOverlayTooltip(null);
        };

        map.on("click", OVERLAY_LAYER_ID, handleOverlayClick);
        map.on("mouseenter", OVERLAY_LAYER_ID, handleOverlayMouseEnter);
        map.on("mousemove", OVERLAY_LAYER_ID, handleOverlayMouseMove);
        map.on("mouseleave", OVERLAY_LAYER_ID, handleOverlayMouseLeave);

        overlayClickHandlerRef.current = handleOverlayClick;
        overlayMouseEnterHandlerRef.current = handleOverlayMouseEnter;
        overlayMouseMoveHandlerRef.current = handleOverlayMouseMove;
        overlayMouseLeaveHandlerRef.current = handleOverlayMouseLeave;
      } else {
        map.getCanvas().style.cursor = "";
        setOverlayTooltip(null);
      }

      if (shouldRestoreViewRef.current && preservedViewStateRef.current) {
        map.jumpTo(preservedViewStateRef.current);
        shouldRestoreViewRef.current = false;
        preservedViewStateRef.current = null;
        return;
      }

      if (!shouldFitToGeometry) {
        return;
      }

      const explicitFitBoundsPoints = fitBoundsPoints ?? null;

      fitMapToGeometry(
        map,
        explicitFitBoundsPoints ?? routePoints ?? [],
        explicitFitBoundsPoints
          ? []
          : overlays.map((overlay) => overlay.points),
        fitBoundsPadding,
        fitBoundsMaxZoom,
      );
      hasFittedInitialViewRef.current = true;
      lastFittedRoutePointsRef.current = routePoints;
      lastFittedFitBoundsPointsRef.current = fitBoundsPoints;
      lastFitBoundsKeyRef.current = fitBoundsKey;
    };

    if (map.isStyleLoaded()) {
      syncMap();
      return undefined;
    }

    map.once("load", syncMap);

    return () => {
      map.off("load", syncMap);
      setOverlayTooltip(null);
    };
  }, [
    endpointSourceData,
    fitBoundsKey,
    fitBoundsPoints,
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
      const markerSource = map.getSource(MARKER_SOURCE_ID) as GeoJSONSource;

      if (markerAnimationFrameRef.current != null) {
        cancelAnimationFrame(markerAnimationFrameRef.current);
        markerAnimationFrameRef.current = null;
      }

      const now = performance.now();
      const previousMarkersById = new Map(
        renderedMovingMarkersRef.current
          .filter((marker) => marker.point)
          .map((marker) => [marker.id, marker]),
      );
      const previousMarkerUpdateAt = lastMovingMarkerUpdateAtRef.current;
      const keyframeIntervalMs =
        previousMarkerUpdateAt == null
          ? 0
          : Math.max(now - previousMarkerUpdateAt, 0);
      const transitionMs = Math.min(
        keyframeIntervalMs,
        Math.max(movingMarkerTransitionMs, 0),
      );
      const canAnimateMarkers =
        transitionMs > 0 &&
        (routePoints?.length ?? 0) >= 2 &&
        movingMarkers.some((marker) => {
          const previousMarker = previousMarkersById.get(marker.id);

          return (
            previousMarker?.point != null &&
            marker.point != null &&
            typeof previousMarker.progress === "number" &&
            typeof marker.progress === "number"
          );
        });

      lastMovingMarkerUpdateAtRef.current = now;

      if (!canAnimateMarkers) {
        markerSource.setData(markerSourceData);
        renderedMovingMarkersRef.current = movingMarkers;
      } else if (transitionMs < MIN_MARKER_ANIMATION_FRAME_MS) {
        markerSource.setData(markerSourceData);
        renderedMovingMarkersRef.current = movingMarkers;
      } else {
        const animationStartedAt = now;

        const step = (timestamp: number) => {
          const progress = Math.min(
            Math.max((timestamp - animationStartedAt) / transitionMs, 0),
            1,
          );
          const interpolatedMarkers = movingMarkers.map((marker) => {
            const previousMarker = previousMarkersById.get(marker.id);

            if (!previousMarker) {
              return marker;
            }

            return interpolateMovingMarker(
              routePoints,
              previousMarker,
              marker,
              progress,
            );
          });

          markerSource.setData(buildMarkerSourceData(interpolatedMarkers));
          renderedMovingMarkersRef.current = interpolatedMarkers;

          if (progress >= 1) {
            renderedMovingMarkersRef.current = movingMarkers;
            markerAnimationFrameRef.current = null;
            return;
          }

          markerAnimationFrameRef.current = requestAnimationFrame(step);
        };

        markerAnimationFrameRef.current = requestAnimationFrame(step);
      }

      if (!followViewport?.point) {
        lastFollowViewportKeyRef.current = null;
        smoothedFollowTargetRef.current = null;
        return;
      }

      if (isUserChangingZoomRef.current) {
        return;
      }

      const targetZoom = resolveFollowViewportZoom({
        requestedZoom: followViewport.zoom,
        preserveUserZoom: followViewportPreserveUserZoom,
        userFollowZoom: userFollowZoomRef.current,
      });

      const nextFollowViewportKey = [
        followViewport.point.longitude,
        followViewport.point.latitude,
        targetZoom,
      ].join(":");

      if (lastFollowViewportKeyRef.current === nextFollowViewportKey) {
        return;
      }

      const nextTarget = {
        center: [
          followViewport.point.longitude,
          followViewport.point.latitude,
        ] as [number, number],
        zoom: targetZoom,
      };

      if (
        followViewportBehavior === "jump" ||
        lastFollowViewportKeyRef.current == null
      ) {
        smoothedFollowTargetRef.current = nextTarget;
        map.jumpTo(nextTarget);
      } else {
        const smoothedTarget = smoothFollowCameraTarget(
          smoothedFollowTargetRef.current,
          nextTarget,
        );

        smoothedFollowTargetRef.current = smoothedTarget;
        map.easeTo({
          ...smoothedTarget,
          duration: FOLLOW_VIEWPORT_EASE_DURATION_MS,
          easing: (value) => 1 - (1 - value) * (1 - value),
        });
      }

      lastFollowViewportKeyRef.current = nextFollowViewportKey;
    };

    if (map.isStyleLoaded()) {
      syncMarkers();
      return undefined;
    }

    map.once("load", syncMarkers);

    return () => {
      if (markerAnimationFrameRef.current != null) {
        cancelAnimationFrame(markerAnimationFrameRef.current);
        markerAnimationFrameRef.current = null;
      }
      map.off("load", syncMarkers);
    };
  }, [
    followViewport,
    followViewportBehavior,
    followViewportPreserveUserZoom,
    markerSourceData,
    mapStyleKey,
    movingMarkerTransitionMs,
    movingMarkers,
    routePoints,
    showBaseTiles,
  ]);

  return (
    <div className="relative h-full w-full">
      {canShowLayerPicker ? (
        <div
          className={
            layerPickerClassName ??
            "absolute left-3 top-3 z-10 flex flex-wrap gap-2 rounded-box border border-base-300/80 bg-base-100/90 p-1 shadow-sm backdrop-blur"
          }
        >
          {availableBasemaps.map((basemap) => {
            const isActive = basemap === selectedBasemap;

            return (
              <button
                key={basemap}
                type="button"
                className={`btn btn-xs ${isActive ? "btn-primary" : "btn-ghost"}`}
                aria-pressed={isActive}
                onClick={() => {
                  handleBasemapChange(basemap);
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

      {overlayTooltip ? (
        <div
          className="pointer-events-none absolute z-10 -translate-x-1/2 -translate-y-[calc(100%+0.75rem)] rounded-box border border-base-300 bg-base-100 px-3 py-2 text-xs font-medium text-base-content shadow-lg"
          style={{ left: overlayTooltip.x, top: overlayTooltip.y }}
        >
          {overlayTooltip.label}
        </div>
      ) : null}
    </div>
  );
}
