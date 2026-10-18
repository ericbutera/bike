import type { FeatureCollection, LineString } from "geojson";
import type { Map, GeoJSONSource } from "../lib/maplibre";
import type { RouteOverlay } from "./RouteMapTypes";

export const CLIMB_GRADIENT_COLORS = ["#fb923c", "#f43f5e", "#a855f7"] as const;
const SOURCE_ID = "activity-route-climbs";
const ARROW_IMAGE_ID = "climb-direction-arrow";
const LINE_OFFSET = 7;

function directionArrowImage() {
  const size = 16;
  const data = new Uint8Array(size * size * 4);
  for (let y = 2; y < size - 2; y++) {
    for (let x = 0; x < size; x++) {
      const distance = Math.abs(x - (12 - Math.abs(y - 7.5)));
      if (distance > 3) continue;
      const color = distance <= 1.5 ? [255, 255, 255, 255] : [107, 33, 78, 255];
      data.set(color, (y * size + x) * 4);
    }
  }
  return { width: size, height: size, data };
}

export function buildClimbSourceData(
  overlays: RouteOverlay[],
): FeatureCollection<LineString> {
  return {
    type: "FeatureCollection",
    features: overlays
      .filter((overlay) => overlay.points.length >= 2)
      .map((overlay) => ({
        type: "Feature",
        id: overlay.id,
        properties: { label: overlay.label ?? "" },
        geometry: {
          type: "LineString",
          coordinates: overlay.points.map((point) => [
            point.longitude,
            point.latitude,
          ]),
        },
      })),
  };
}

export function syncClimbLayers(map: Map, data: FeatureCollection<LineString>) {
  if (!map.getSource(SOURCE_ID) && data.features.length === 0) return;
  if (!map.getSource(SOURCE_ID)) {
    map.addSource(SOURCE_ID, { type: "geojson", lineMetrics: true, data });
    const beforeLabels = map
      .getStyle()
      .layers?.find((layer) => layer.type === "symbol")?.id;
    map.addLayer(
      {
        id: `${SOURCE_ID}-casing`,
        type: "line",
        source: SOURCE_ID,
        layout: { "line-cap": "round", "line-join": "round" },
        paint: {
          "line-color": "#ffffff",
          "line-width": 7,
          "line-offset": LINE_OFFSET,
        },
      },
      beforeLabels,
    );
    map.addLayer(
      {
        id: `${SOURCE_ID}-line`,
        type: "line",
        source: SOURCE_ID,
        layout: { "line-cap": "round", "line-join": "round" },
        paint: {
          "line-width": 4,
          "line-offset": LINE_OFFSET,
          "line-gradient": [
            "interpolate",
            ["linear"],
            ["line-progress"],
            0,
            CLIMB_GRADIENT_COLORS[0],
            0.5,
            CLIMB_GRADIENT_COLORS[1],
            1,
            CLIMB_GRADIENT_COLORS[2],
          ],
        },
      },
      beforeLabels,
    );
    if (!map.hasImage(ARROW_IMAGE_ID))
      map.addImage(ARROW_IMAGE_ID, directionArrowImage());
    map.addLayer({
      id: `${SOURCE_ID}-arrows`,
      type: "symbol",
      source: SOURCE_ID,
      layout: {
        "symbol-placement": "line",
        "symbol-spacing": 64,
        "icon-image": ARROW_IMAGE_ID,
        "icon-rotation-alignment": "map",
        "icon-keep-upright": false,
        "icon-offset": [0, LINE_OFFSET],
        "icon-allow-overlap": true,
        "icon-ignore-placement": true,
      },
    });
  }
  (map.getSource(SOURCE_ID) as GeoJSONSource).setData(data);
}
