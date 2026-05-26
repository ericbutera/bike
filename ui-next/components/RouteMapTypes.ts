import { type ActivityRoutePoint } from "../lib/queries";

export type RouteMapBasemap = "topo" | "street" | "satellite";

export type RouteMapFollowViewport = {
  point: ActivityRoutePoint;
  zoom: number;
};

export type RouteMapFollowViewportBehavior = "ease" | "jump";

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
  progress?: number | null;
  color: string;
  opacity?: number;
  label?: string;
};

export type RouteMapProps = {
  routePoints: ActivityRoutePoint[] | null | undefined;
  overlays?: RouteOverlay[];
  movingMarkers?: RouteMovingMarker[];
  movingMarkerTransitionMs?: number;
  followViewport?: RouteMapFollowViewport | null;
  followViewportBehavior?: RouteMapFollowViewportBehavior;
  layerPickerClassName?: string;
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
  showRouteEndpoints?: boolean;
};
