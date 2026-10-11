export type RouteMapBasemap =
  "default" | "topo" | "street" | "satellite" | "route-light" | "fiord";

export const DEFAULT_ROUTE_COLOR = "#0060df";
export const DEFAULT_ROUTE_BASEMAP = "default";

export const ROUTE_MAP_BASEMAP_LABELS: Record<RouteMapBasemap, string> = {
  default: "Map",
  topo: "Topo",
  street: "Street",
  satellite: "Satellite",
  "route-light": "Route light",
  fiord: "Fiord",
};

export const ROUTE_MAP_BASEMAP_OPTIONS: RouteMapBasemap[] = [
  DEFAULT_ROUTE_BASEMAP,
  "topo",
  "street",
  "satellite",
];

export function bikeMapStyle(theme: string): string {
  return theme === "dark"
    ? "/map-styles/fiord-v1.json"
    : "/map-styles/route-light-v1.json";
}
