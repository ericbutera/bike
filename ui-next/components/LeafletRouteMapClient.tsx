"use client";

import L from "leaflet";
import { useEffect, useMemo, useRef } from "react";
import { type ActivityRoutePoint } from "../lib/queries";

type RouteOverlay = {
  id: string;
  points: ActivityRoutePoint[];
  color: string;
  weight?: number;
  onClick?: () => void;
};

export type LeafletRouteMapProps = {
  routePoints: ActivityRoutePoint[] | null | undefined;
  overlays?: RouteOverlay[];
  movingMarkers?: Array<{
    id: string;
    point: ActivityRoutePoint | null;
    color: string;
    opacity?: number;
  }>;
  ariaLabel: string;
  className?: string;
  emptyMessage: string;
  showBaseTiles?: boolean;
  interactive?: boolean;
};

function toLatLngs(points: ActivityRoutePoint[]) {
  return points.map(
    (point) => [point.latitude, point.longitude] as [number, number],
  );
}

export default function LeafletRouteMapClient({
  routePoints,
  overlays = [],
  movingMarkers = [],
  showBaseTiles = true,
  interactive = true,
  ariaLabel: _ariaLabel,
  className: _className,
  emptyMessage: _emptyMessage,
}: LeafletRouteMapProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const mapRef = useRef<L.Map | null>(null);
  const routeLayerGroupRef = useRef<L.LayerGroup | null>(null);
  const markerLayerGroupRef = useRef<L.LayerGroup | null>(null);
  const lastFittedGeometryKeyRef = useRef<string | null>(null);
  const resizeFrameRef = useRef<number | null>(null);
  const fitBoundsFrameRef = useRef<number | null>(null);
  const routeLatLngs = useMemo(
    () => toLatLngs(routePoints ?? []),
    [routePoints],
  );
  const overlayLatLngs = useMemo(
    () =>
      overlays.map((overlay) => ({
        ...overlay,
        latLngs: toLatLngs(overlay.points ?? []),
      })),
    [overlays],
  );
  const geometryKey = useMemo(() => {
    const routeStart = routeLatLngs[0];
    const routeEnd = routeLatLngs.at(-1);

    return [
      routeLatLngs.length,
      routeStart?.join(",") ?? "",
      routeEnd?.join(",") ?? "",
      ...overlayLatLngs.map((overlay) => {
        const first = overlay.latLngs[0];
        const last = overlay.latLngs.at(-1);

        return [
          overlay.id,
          overlay.latLngs.length,
          first?.join(",") ?? "",
          last?.join(",") ?? "",
          overlay.color,
          overlay.weight ?? "",
        ].join(":");
      }),
    ].join("|");
  }, [overlayLatLngs, routeLatLngs]);

  useEffect(() => {
    if (routeLatLngs.length < 2 || !containerRef.current || mapRef.current) {
      return undefined;
    }

    const map = L.map(containerRef.current, {
      zoomControl: interactive,
      attributionControl: showBaseTiles,
      scrollWheelZoom: interactive,
      dragging: interactive,
      doubleClickZoom: interactive,
      boxZoom: interactive,
      keyboard: interactive,
      touchZoom: interactive,
      zoomSnap: 0.1,
    });

    if (showBaseTiles) {
      L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {
        attribution: "&copy; OpenStreetMap contributors",
        maxZoom: 19,
      }).addTo(map);
    }

    const routeLayerGroup = L.layerGroup().addTo(map);
    const markerLayerGroup = L.layerGroup().addTo(map);
    const handleResize = () => {
      if (resizeFrameRef.current != null) {
        cancelAnimationFrame(resizeFrameRef.current);
      }

      resizeFrameRef.current = requestAnimationFrame(() => {
        if (mapRef.current !== map || !containerRef.current?.isConnected) {
          return;
        }

        map.invalidateSize?.();
      });
    };

    mapRef.current = map;
    routeLayerGroupRef.current = routeLayerGroup;
    markerLayerGroupRef.current = markerLayerGroup;

    window.addEventListener("resize", handleResize);
    document.addEventListener("fullscreenchange", handleResize);

    resizeFrameRef.current = requestAnimationFrame(() => {
      if (mapRef.current !== map || !containerRef.current?.isConnected) {
        return;
      }

      map.invalidateSize?.();
    });

    return () => {
      window.removeEventListener("resize", handleResize);
      document.removeEventListener("fullscreenchange", handleResize);
      if (resizeFrameRef.current != null) {
        cancelAnimationFrame(resizeFrameRef.current);
        resizeFrameRef.current = null;
      }
      if (fitBoundsFrameRef.current != null) {
        cancelAnimationFrame(fitBoundsFrameRef.current);
        fitBoundsFrameRef.current = null;
      }
      routeLayerGroup.clearLayers();
      markerLayerGroup.clearLayers();
      map.remove();
      mapRef.current = null;
      routeLayerGroupRef.current = null;
      markerLayerGroupRef.current = null;
      lastFittedGeometryKeyRef.current = null;
    };
  }, [interactive, routeLatLngs.length, showBaseTiles]);

  useEffect(() => {
    const map = mapRef.current;
    const routeLayerGroup = routeLayerGroupRef.current;
    if (!map || !routeLayerGroup) {
      return;
    }

    routeLayerGroup.clearLayers();

    if (routeLatLngs.length < 2) {
      lastFittedGeometryKeyRef.current = null;
      map.setView([37.7749, -122.4194], 10);
      return;
    }

    const baseRoute = L.polyline(routeLatLngs, {
      color: "#1f2937",
      weight: 4,
      opacity: 0.9,
      lineCap: "round",
      lineJoin: "round",
    }).addTo(routeLayerGroup);
    const bounds = baseRoute.getBounds();

    L.circleMarker(routeLatLngs[0], {
      radius: 6,
      color: "#111827",
      weight: 2,
      fillColor: "#111827",
      fillOpacity: 1,
    }).addTo(routeLayerGroup);

    L.circleMarker(routeLatLngs[routeLatLngs.length - 1], {
      radius: 6,
      color: "#111827",
      weight: 2,
      fillColor: "#ffffff",
      fillOpacity: 1,
    }).addTo(routeLayerGroup);

    for (const overlay of overlayLatLngs) {
      if (overlay.latLngs.length < 2) {
        continue;
      }

      const overlayLine = L.polyline(overlay.latLngs, {
        color: overlay.color,
        weight: overlay.weight ?? 6,
        opacity: 0.95,
        lineCap: "round",
        lineJoin: "round",
        className: overlay.onClick ? "cursor-pointer" : undefined,
      }).addTo(routeLayerGroup);

      if (
        overlay.onClick &&
        typeof (overlayLine as { on?: unknown }).on === "function"
      ) {
        overlayLine.on("click", overlay.onClick);
      }

      if (typeof (bounds as { extend?: unknown }).extend === "function") {
        bounds.extend(overlayLine.getBounds());
      }
    }

    if (fitBoundsFrameRef.current != null) {
      cancelAnimationFrame(fitBoundsFrameRef.current);
    }

    fitBoundsFrameRef.current = requestAnimationFrame(() => {
      if (mapRef.current !== map || !containerRef.current?.isConnected) {
        return;
      }

      map.invalidateSize?.();

      if (lastFittedGeometryKeyRef.current !== geometryKey) {
        map.fitBounds(bounds, {
          padding: [8, 8],
          maxZoom: 19,
        });
        lastFittedGeometryKeyRef.current = geometryKey;
      }
    });

    return () => {
      if (fitBoundsFrameRef.current != null) {
        cancelAnimationFrame(fitBoundsFrameRef.current);
        fitBoundsFrameRef.current = null;
      }
    };
  }, [geometryKey, overlayLatLngs, routeLatLngs]);

  useEffect(() => {
    const markerLayerGroup = markerLayerGroupRef.current;
    if (!markerLayerGroup) {
      return;
    }

    markerLayerGroup.clearLayers();

    for (const marker of movingMarkers) {
      if (!marker.point) {
        continue;
      }

      const opacity = marker.opacity ?? 1;

      L.circleMarker([marker.point.latitude, marker.point.longitude], {
        radius: 7,
        color: marker.color,
        weight: 2,
        opacity,
        fillColor: marker.color,
        fillOpacity: opacity,
      }).addTo(markerLayerGroup);
    }
  }, [movingMarkers]);

  if (routeLatLngs.length < 2) {
    return null;
  }

  return <div ref={containerRef} className="h-full w-full" />;
}
