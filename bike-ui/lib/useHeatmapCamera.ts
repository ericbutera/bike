import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type RefObject,
} from "react";
import type {
  GeoJSONSource,
  Map,
  MapLayerMouseEvent,
  MapMovementEvent,
} from "maplibre-gl";
import {
  DEFAULT_HEATMAP_ZOOM,
  HEATMAP_ZONE_LAYERS,
  HEATMAP_ZONES_SOURCE,
  localRidingCenter,
  type HeatmapCamera,
  type HeatmapView,
} from "./heatmapCamera";
import type { HeatmapMetadata, HeatmapZones } from "./heatmaps";
import { currentMapLocation, locateMapRegion } from "./mapRegions";

function fitBounds(map: Map, bounds: number[]) {
  const [west, south, east, north] = bounds;
  map.fitBounds(
    [
      [west, south],
      [east, north],
    ],
    { padding: 50, maxZoom: 14, duration: 0 },
  );
}

async function zoomToZone(
  map: Map,
  event: MapLayerMouseEvent,
  signal: AbortSignal,
) {
  const feature = event.features?.[0];
  if (!feature || feature.geometry.type !== "Point") return;
  const center = feature.geometry.coordinates as [number, number];
  const clusterId = feature.properties?.cluster_id;
  let zoom = Math.min(18, Math.max(map.getZoom() + 2, 8));
  if (typeof clusterId === "number") {
    const source = map.getSource(HEATMAP_ZONES_SOURCE) as
      GeoJSONSource | undefined;
    if (!source) return;
    const expansionZoom = await source.getClusterExpansionZoom(clusterId);
    zoom = Math.min(18, Math.max(expansionZoom, map.getZoom() + 2));
  }
  if (!signal.aborted) map.easeTo({ center, zoom, duration: 600 });
}

export function useHeatmapCamera(
  map: RefObject<Map | null>,
  view: HeatmapView,
  routes: { metadata?: HeatmapMetadata; zones: HeatmapZones["zones"] },
) {
  const positioned = useRef(Boolean(view.camera));
  const pending = useRef<AbortController | null>(null);
  const publishedCamera = useRef<HeatmapCamera | undefined>(undefined);
  const [locationUnavailable, setLocationUnavailable] = useState(false);
  const current = useRef({ view, routes });
  useEffect(() => {
    current.current = { view, routes };
  }, [view, routes]);

  // Every explicit camera command supersedes older location/region/cluster work.
  const beginViewChange = useCallback(() => {
    positioned.current = true;
    pending.current?.abort();
    const controller = new AbortController();
    pending.current = controller;
    return controller;
  }, []);

  useEffect(() => {
    const instance = map.current;
    if (!instance) return;
    const moved = (event?: MapMovementEvent) => {
      if (!event?.originalEvent) return;
      positioned.current = true;
      pending.current?.abort();
    };
    const settled = () => {
      if (!positioned.current) return;
      const center = instance.getCenter().wrap();
      const camera = {
        longitude: Number(center.lng.toFixed(6)),
        latitude: Number(center.lat.toFixed(6)),
        zoom: Number(instance.getZoom().toFixed(2)),
      };
      publishedCamera.current = camera;
      current.current.view.onChange(camera);
    };
    instance.on("movestart", moved);
    instance.on("move", moved);
    instance.on("moveend", settled);
    return () => {
      instance.off("movestart", moved);
      instance.off("move", moved);
      instance.off("moveend", settled);
      pending.current?.abort();
    };
  }, [map]);

  const { longitude, latitude, zoom } = view.camera ?? {};
  useEffect(() => {
    const instance = map.current;
    if (
      !instance ||
      longitude === undefined ||
      latitude === undefined ||
      zoom === undefined
    )
      return;
    // URL persistence is an output of movement, not another camera command.
    const published = publishedCamera.current;
    publishedCamera.current = undefined;
    if (
      published?.longitude === longitude &&
      published.latitude === latitude &&
      published.zoom === zoom
    )
      return;
    beginViewChange();
    const center = instance.getCenter().wrap();
    if (
      Math.abs(center.lng - longitude) < 0.000001 &&
      Math.abs(center.lat - latitude) < 0.000001 &&
      Math.abs(instance.getZoom() - zoom) < 0.01
    )
      return;
    instance.jumpTo({
      center: [longitude, latitude],
      zoom,
    });
  }, [map, longitude, latitude, zoom, beginViewChange]);

  useEffect(() => {
    const instance = map.current;
    if (!instance || positioned.current || current.current.view.request) return;
    const controller = new AbortController();
    pending.current = controller;
    void currentMapLocation()
      .then((position) => {
        if (controller.signal.aborted || positioned.current) return;
        positioned.current = true;
        instance.jumpTo({
          center: [position.longitude, position.latitude],
          zoom: DEFAULT_HEATMAP_ZOOM,
        });
      })
      .catch(() => {
        if (!controller.signal.aborted && !positioned.current)
          setLocationUnavailable(true);
      });
    return () => {
      controller.abort();
    };
  }, [map]);

  useEffect(() => {
    const center = localRidingCenter(routes.zones);
    if (!map.current || positioned.current || !locationUnavailable || !center)
      return;
    beginViewChange();
    map.current.jumpTo({ center, zoom: DEFAULT_HEATMAP_ZOOM });
  }, [map, routes.zones, locationUnavailable, beginViewChange]);

  const action = view.request?.action;
  const sequence = view.request?.sequence;
  useEffect(() => {
    const instance = map.current;
    if (!instance || !action) return;
    const controller = beginViewChange();
    const apply = async () => {
      current.current.view.onError(undefined);
      if (action === "full") {
        const bounds = current.current.routes.metadata?.bounds;
        if (bounds) fitBounds(instance, bounds);
        return;
      }
      const position = await currentMapLocation();
      if (controller.signal.aborted) return;
      const center: [number, number] = [position.longitude, position.latitude];
      if (action === "location") {
        instance.flyTo({ center, zoom: DEFAULT_HEATMAP_ZOOM, duration: 900 });
      } else {
        const region = await locateMapRegion(center, controller.signal);
        if (!controller.signal.aborted) fitBounds(instance, region.bounds);
      }
    };
    void apply().catch((error: unknown) => {
      if (!controller.signal.aborted)
        current.current.view.onError(
          error instanceof Error
            ? error.message
            : "Could not change the map view. Try again.",
        );
    });
    return () => controller.abort();
  }, [map, action, sequence, beginViewChange]);

  useEffect(() => {
    const instance = map.current;
    if (!instance) return;
    const selectZone = (event: MapLayerMouseEvent) => {
      if (event.features?.[0]?.geometry.type !== "Point") return;
      const controller = beginViewChange();
      current.current.view.onError(undefined);
      void zoomToZone(instance, event, controller.signal).catch(() => {
        if (!controller.signal.aborted)
          current.current.view.onError(
            "Could not zoom into that route group. Try again.",
          );
      });
    };
    for (const layer of HEATMAP_ZONE_LAYERS)
      instance.on("click", layer, selectZone);
    return () => {
      for (const layer of HEATMAP_ZONE_LAYERS)
        instance.off("click", layer, selectZone);
    };
  }, [map, beginViewChange]);
}
