import type {
  Feature,
  FeatureCollection,
  MultiPolygon,
  Polygon,
  Position,
} from "geojson";

type RegionFeature = Feature<Polygon | MultiPolygon, { name: string }>;
export type MapRegions = FeatureCollection<
  Polygon | MultiPolygon,
  { name: string }
>;
export type MapRegion = {
  name: string;
  bounds: [number, number, number, number];
};
let regions: MapRegions | undefined;

function containsRing(
  ring: Position[],
  [longitude, latitude]: Position,
): boolean {
  let inside = false;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    const [xi, yi] = ring[i];
    const [xj, yj] = ring[j];
    if (
      yi > latitude !== yj > latitude &&
      longitude < ((xj - xi) * (latitude - yi)) / (yj - yi) + xi
    )
      inside = !inside;
  }
  return inside;
}

function containsRegion(feature: RegionFeature, position: Position): boolean {
  const polygons =
    feature.geometry.type === "Polygon"
      ? [feature.geometry.coordinates]
      : feature.geometry.coordinates;
  return polygons.some(
    ([outer, ...holes]) =>
      containsRing(outer, position) &&
      !holes.some((hole) => containsRing(hole, position)),
  );
}

function regionBounds(points: Position[]): MapRegion["bounds"] {
  const longitudes = points
    .map(([longitude]) => longitude)
    .sort((a, b) => a - b);
  let west = longitudes[0];
  let east = longitudes.at(-1)!;
  if (east - west > 180) {
    let largestGap = 0;
    for (let i = 0; i < longitudes.length; i++) {
      const next = longitudes[i + 1] ?? longitudes[0] + 360;
      if (next - longitudes[i] > largestGap) {
        largestGap = next - longitudes[i];
        west = next > 180 ? next - 360 : next;
        east = longitudes[i];
      }
    }
    if (east < west) east += 360;
  }
  let south = Infinity;
  let north = -Infinity;
  for (const [, latitude] of points) {
    south = Math.min(south, latitude);
    north = Math.max(north, latitude);
  }
  return [west, south, east, north];
}

export function findMapRegion(
  data: MapRegions,
  position: Position,
): MapRegion | undefined {
  const feature = data.features.find((item) => containsRegion(item, position));
  if (!feature) return;
  const points =
    feature.geometry.type === "Polygon"
      ? feature.geometry.coordinates.flat()
      : feature.geometry.coordinates.flat(2);
  return { name: feature.properties.name, bounds: regionBounds(points) };
}

export async function locateMapRegion(
  position: Position,
  signal: AbortSignal,
): Promise<MapRegion> {
  if (!regions) {
    const response = await fetch("/map-regions.geojson.gz", {
      signal: AbortSignal.any([signal, AbortSignal.timeout(10_000)]),
    });
    if (!response.ok)
      throw new Error("State boundaries could not load. Try again.");
    if (!response.body)
      throw new Error("State boundaries could not load. Try again.");
    const decoded = response.body.pipeThrough(new DecompressionStream("gzip"));
    regions = (await new Response(decoded).json()) as MapRegions;
  }
  const region = findMapRegion(regions, position);
  if (!region)
    throw new Error(
      "Could not determine your state from this location. Try again.",
    );
  return region;
}

export function currentMapLocation(): Promise<GeolocationCoordinates> {
  return new Promise((resolve, reject) => {
    if (!navigator.geolocation) {
      reject(new Error("Location is not available in this browser."));
      return;
    }
    navigator.geolocation.getCurrentPosition(
      ({ coords }) => resolve(coords),
      (error) =>
        reject(
          new Error(
            error.code === 1
              ? "Location access was denied. Allow location access in your browser and try again."
              : "Could not determine your location. Try again.",
          ),
        ),
      { enableHighAccuracy: false, maximumAge: 300_000, timeout: 10_000 },
    );
  });
}
