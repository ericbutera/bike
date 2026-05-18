import { type ActivityRoutePoint } from "../lib/queries";

export type RouteMapBasemap = "topo" | "street" | "satellite";

export type RouteOverlay = {
  id: string;
  points: ActivityRoutePoint[];
  color: string;
  weight?: number;
  onClick?: () => void;
};

export type RouteMovingMarker = {
  id: string;
  point: ActivityRoutePoint | null;
  color: string;
  opacity?: number;
};

export type RouteMapProps = {
  routePoints: ActivityRoutePoint[] | null | undefined;
  overlays?: RouteOverlay[];
  movingMarkers?: RouteMovingMarker[];
  ariaLabel: string;
  className?: string;
  emptyMessage: string;
  showBaseTiles?: boolean;
  interactive?: boolean;
  showZoomControls?: boolean;
  showLayerPicker?: boolean;
  basemapOptions?: RouteMapBasemap[];
  defaultBasemap?: RouteMapBasemap;
  fitBoundsPadding?: number;
  fitBoundsMaxZoom?: number;
};
