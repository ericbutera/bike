import { icon } from "@fortawesome/fontawesome-svg-core";
import { faFlagCheckered, faPlay } from "@fortawesome/free-solid-svg-icons";
import { Marker, type Map } from "../lib/maplibre";
import type { ActivityRoutePoint } from "../lib/queries";

type EndpointKind = "start" | "finish";

function endpointElement(kind: EndpointKind) {
  const isStart = kind === "start";
  const element = document.createElement("div");
  element.className = "pointer-events-none h-4 w-4";
  element.setAttribute("role", "img");
  element.setAttribute("aria-label", `Route ${kind}`);

  element.innerHTML = icon(isStart ? faPlay : faFlagCheckered, {
    styles: {
      display: "block",
      width: "16px",
      height: "16px",
      color: isStart ? "#16a34a" : "#0f172a",
      stroke: "#ffffff",
      "stroke-width": "24",
      "stroke-linejoin": "round",
      "paint-order": "stroke",
    },
    attributes: { "aria-hidden": "true" },
  }).html.join("");
  return element;
}

export default class RouteMapEndpoints {
  private markers: Marker[] = [];
  private routePoints: ActivityRoutePoint[] | null | undefined;

  constructor(private readonly map: Map) {}

  setRoute(routePoints: ActivityRoutePoint[] | null | undefined) {
    if (this.routePoints === routePoints) return;
    this.routePoints = routePoints;
    if (!routePoints || routePoints.length < 2) {
      this.remove();
      return;
    }
    const points = [routePoints[0], routePoints.at(-1)!];
    (["start", "finish"] as const).forEach((kind, index) => {
      const point = points[index];
      const marker =
        this.markers[index] ??
        new Marker({
          element: endpointElement(kind),
          anchor: "center",
        });
      marker.setLngLat([point.longitude, point.latitude]);
      if (!this.markers[index]) {
        marker.addTo(this.map);
        this.markers.push(marker);
      }
    });
  }

  remove() {
    this.markers.forEach((marker) => marker.remove());
    this.markers = [];
    this.routePoints = undefined;
  }
}
