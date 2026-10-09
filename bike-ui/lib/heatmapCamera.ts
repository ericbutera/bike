import type { HeatmapZones } from "./heatmaps";

export const DEFAULT_HEATMAP_ZOOM = 13;
export const HEATMAP_ZONES_SOURCE = "personal-heatmap-zones";
export const HEATMAP_ZONE_LAYERS = [
  "personal-heatmap-zone-circles",
  "personal-heatmap-single-zone-circles",
] as const;

export type HeatmapCamera = {
  longitude: number;
  latitude: number;
  zoom: number;
};
export const HEATMAP_LOADING_CAMERA: HeatmapCamera = {
  longitude: 0,
  latitude: 0,
  zoom: 2,
};
export type HeatmapViewAction = "region" | "full" | "location";
export type HeatmapView = {
  camera?: HeatmapCamera;
  request?: { action: HeatmapViewAction; sequence: number };
  onChange: (camera: HeatmapCamera) => void;
  onRequest: (action: HeatmapViewAction) => void;
  onError: (message?: string) => void;
};

export function parseHeatmapCamera(
  params: URLSearchParams,
): HeatmapCamera | undefined {
  const values = ["lng", "lat", "zoom"].map((key) => params.get(key));
  if (values.some((value) => value === null || value.trim() === "")) return;
  const [longitude, latitude, zoom] = values.map(Number);
  if (
    ![longitude, latitude, zoom].every(Number.isFinite) ||
    longitude < -180 ||
    longitude > 180 ||
    latitude < -85 ||
    latitude > 85 ||
    zoom < 0 ||
    zoom > 18
  )
    return;
  // Recover loading coordinates that older clients persisted before routes arrived.
  if (
    longitude === HEATMAP_LOADING_CAMERA.longitude &&
    latitude === HEATMAP_LOADING_CAMERA.latitude &&
    zoom === HEATMAP_LOADING_CAMERA.zoom
  )
    return;
  return { longitude, latitude, zoom };
}

export function localRidingCenter(
  zones: HeatmapZones["zones"],
): [number, number] | undefined {
  const groups = new Map<
    string,
    { longitude: number; latitude: number; count: number }
  >();
  for (const zone of zones) {
    const key = `${Math.floor(zone.longitude * 4)},${Math.floor(zone.latitude * 4)}`;
    const group = groups.get(key) ?? { longitude: 0, latitude: 0, count: 0 };
    group.longitude += zone.longitude;
    group.latitude += zone.latitude;
    group.count++;
    groups.set(key, group);
  }
  const busiest = [...groups.values()].sort((a, b) => b.count - a.count)[0];
  return busiest
    ? [busiest.longitude / busiest.count, busiest.latitude / busiest.count]
    : undefined;
}
