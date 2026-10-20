import { afterEach, describe, expect, it, vi } from "vitest";
import { climbingRoutePoints } from "../tests/fixtures/activities";
import { segmentEffort } from "../tests/fixtures/activitySegments";
import { downloadFile } from "./downloadFile";
import { downloadSegmentGpx, segmentGpx } from "./segmentGpx";

vi.mock("./downloadFile", () => ({ downloadFile: vi.fn() }));

const { segment_id: id, segment_title: title } = segmentEffort();
const segment = { id, title, route_points: climbingRoutePoints() };

afterEach(() => vi.clearAllMocks());

describe("segment GPX", () => {
  it("exports a named GPX track with ordered coordinates and available elevations", () => {
    const xml = segmentGpx(segment);
    const document = new DOMParser().parseFromString(xml, "application/xml");

    expect(document.querySelector("parsererror")).toBeNull();
    expect(document.documentElement.namespaceURI).toBe(
      "http://www.topografix.com/GPX/1/1",
    );
    expect(document.documentElement.getAttribute("version")).toBe("1.1");
    expect(document.documentElement.getAttribute("creator")).toBe("Bike");
    expect(document.querySelector("trk > name")?.textContent).toBe(
      segment.title,
    );
    const points = [...document.querySelectorAll("trkseg > trkpt")];
    expect(
      points.map((point) => [
        point.getAttribute("lat"),
        point.getAttribute("lon"),
        point.querySelector("ele")?.textContent ?? null,
      ]),
    ).toEqual(
      segment.route_points.map((point) => [
        String(point.latitude),
        String(point.longitude),
        String(point.elevation_meters),
      ]),
    );
    expect(document.querySelectorAll("trkseg")).toHaveLength(1);
    expect(
      document.querySelectorAll("time, extensions, hr, power"),
    ).toHaveLength(0);
  });

  it("escapes special characters in the segment title", () => {
    const title = 'North & South <Climb> "☃"';
    const xml = segmentGpx({ ...segment, title });
    const document = new DOMParser().parseFromString(xml, "application/xml");

    expect(document.querySelector("parsererror")).toBeNull();
    expect(document.querySelector("trk > name")?.textContent).toBe(title);
    expect(document.querySelector("Climb")).toBeNull();
  });

  it.each([0, -4.5])(
    "preserves an elevation of %s meters",
    (elevation_meters) => {
      const xml = segmentGpx({
        ...segment,
        route_points: [{ ...segment.route_points[0], elevation_meters }],
      });
      const document = new DOMParser().parseFromString(xml, "application/xml");

      expect(document.querySelector("ele")?.textContent).toBe(
        String(elevation_meters),
      );
    },
  );

  it.each([null, undefined])(
    "omits unavailable elevations",
    (elevation_meters) => {
      const xml = segmentGpx({
        ...segment,
        route_points: [{ ...segment.route_points[0], elevation_meters }],
      });
      const document = new DOMParser().parseFromString(xml, "application/xml");

      expect(document.querySelectorAll("trkpt")).toHaveLength(1);
      expect(document.querySelector("ele")).toBeNull();
    },
  );

  it("handles absent route points without adding track points", () => {
    const xml = segmentGpx({ ...segment, route_points: undefined });
    expect(
      new DOMParser()
        .parseFromString(xml, "application/xml")
        .querySelectorAll("trkpt"),
    ).toHaveLength(0);
  });

  it("downloads the current segment as a GPX file with a stable filename", async () => {
    downloadSegmentGpx(segment);

    expect(downloadFile).toHaveBeenCalledOnce();
    const [filename, blob] = vi.mocked(downloadFile).mock.calls[0];
    expect(filename).toBe(`segment-${segment.id}.gpx`);
    expect(blob.type).toBe("application/gpx+xml;charset=utf-8");
    expect(await blob.text()).toBe(segmentGpx(segment));
  });

  it.each([undefined, [], [segment.route_points[0]]])(
    "does not download an unavailable route",
    (route_points) => {
      downloadSegmentGpx({ ...segment, route_points });
      expect(downloadFile).not.toHaveBeenCalled();
    },
  );
});
