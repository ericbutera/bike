import { useEffect, useRef, useState, type RefObject } from "react";
import type { Map, MapMovementEvent } from "maplibre-gl";
import {
  DEFAULT_HEATMAP_ZOOM,
  localRidingCenter,
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

export function useHeatmapCamera(
  map: RefObject<Map | null>,
  view: HeatmapView,
  routes: { metadata?: HeatmapMetadata; zones: HeatmapZones["zones"] },
) {
  const positioned = useRef(Boolean(view.camera));
  const [locationUnavailable, setLocationUnavailable] = useState(false);
  const current = useRef({ view, routes });
  useEffect(() => {
    current.current = { view, routes };
  }, [view, routes]);

  useEffect(() => {
    const instance = map.current;
    if (!instance) return;
    const moved = (event?: MapMovementEvent) => {
      if (event?.originalEvent) positioned.current = true;
    };
    const settled = () => {
      if (!positioned.current) return;
      const center = instance.getCenter().wrap();
      current.current.view.onChange({
        longitude: Number(center.lng.toFixed(6)),
        latitude: Number(center.lat.toFixed(6)),
        zoom: Number(instance.getZoom().toFixed(2)),
      });
    };
    instance.on("movestart", moved);
    instance.on("moveend", settled);
    return () => {
      instance.off("movestart", moved);
      instance.off("moveend", settled);
    };
  }, [map]);

  const camera = view.camera;
  useEffect(() => {
    const instance = map.current;
    if (!instance || !camera) return;
    positioned.current = true;
    const center = instance.getCenter().wrap();
    if (
      Math.abs(center.lng - camera.longitude) < 0.000001 &&
      Math.abs(center.lat - camera.latitude) < 0.000001 &&
      Math.abs(instance.getZoom() - camera.zoom) < 0.01
    )
      return;
    instance.jumpTo({
      center: [camera.longitude, camera.latitude],
      zoom: camera.zoom,
    });
  }, [map, camera]);

  useEffect(() => {
    const instance = map.current;
    if (!instance || positioned.current || current.current.view.request) return;
    let active = true;
    void currentMapLocation()
      .then((position) => {
        if (!active || positioned.current) return;
        positioned.current = true;
        instance.jumpTo({
          center: [position.longitude, position.latitude],
          zoom: DEFAULT_HEATMAP_ZOOM,
        });
      })
      .catch(() => {
        if (active && !positioned.current) setLocationUnavailable(true);
      });
    return () => {
      active = false;
    };
  }, [map]);

  useEffect(() => {
    const center = localRidingCenter(routes.zones);
    if (!map.current || positioned.current || !locationUnavailable || !center)
      return;
    positioned.current = true;
    map.current.jumpTo({ center, zoom: DEFAULT_HEATMAP_ZOOM });
  }, [map, routes.zones, locationUnavailable]);

  const request = view.request;
  useEffect(() => {
    const instance = map.current;
    if (!instance || !request) return;
    const controller = new AbortController();
    positioned.current = true;
    const apply = async () => {
      current.current.view.onError(undefined);
      if (request.action === "full") {
        const bounds = current.current.routes.metadata?.bounds;
        if (bounds) fitBounds(instance, bounds);
        return;
      }
      const position = await currentMapLocation();
      if (controller.signal.aborted) return;
      const center: [number, number] = [position.longitude, position.latitude];
      if (request.action === "location") {
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
  }, [map, request]);
}
