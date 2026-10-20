import { downloadFile } from "./downloadFile";
import type { Segment } from "./queries";

const GPX_NAMESPACE = "http://www.topografix.com/GPX/1/1";

export function segmentGpx(segment: Pick<Segment, "title" | "route_points">) {
  const document = new DOMParser().parseFromString(
    `<gpx xmlns="${GPX_NAMESPACE}" version="1.1" creator="Bike"/>`,
    "application/xml",
  );
  const root = document.documentElement;
  const track = document.createElementNS(GPX_NAMESPACE, "trk");
  const name = document.createElementNS(GPX_NAMESPACE, "name");
  name.textContent = segment.title;
  track.append(name);
  const route = document.createElementNS(GPX_NAMESPACE, "trkseg");

  for (const point of segment.route_points ?? []) {
    const trackPoint = document.createElementNS(GPX_NAMESPACE, "trkpt");
    trackPoint.setAttribute("lat", String(point.latitude));
    trackPoint.setAttribute("lon", String(point.longitude));
    if (point.elevation_meters != null) {
      const elevation = document.createElementNS(GPX_NAMESPACE, "ele");
      elevation.textContent = String(point.elevation_meters);
      trackPoint.append(elevation);
    }
    route.append(trackPoint);
  }

  track.append(route);
  root.append(track);
  return (
    '<?xml version="1.0" encoding="UTF-8"?>\n' +
    new XMLSerializer().serializeToString(document)
  );
}

export function downloadSegmentGpx(
  segment: Pick<Segment, "id" | "title" | "route_points">,
) {
  if ((segment.route_points?.length ?? 0) < 2) {
    return;
  }

  downloadFile(
    `segment-${segment.id}.gpx`,
    new Blob([segmentGpx(segment)], {
      type: "application/gpx+xml;charset=utf-8",
    }),
  );
}
