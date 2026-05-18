import dynamic from "next/dynamic";
import { type RouteMapProps } from "./RouteMapTypes";

const MapLibreRouteMapClient = dynamic(
  () => import("./MapLibreRouteMapClient").then((module) => module.default),
  {
    ssr: false,
    loading: () => null,
  },
);

export default function MapLibreRouteMap(props: RouteMapProps) {
  const hasRoute = (props.routePoints?.length ?? 0) >= 2;

  if (!hasRoute) {
    return <div className="alert">{props.emptyMessage}</div>;
  }

  return (
    <div
      className={
        props.className ??
        "h-96 w-full overflow-hidden rounded-box border border-base-300 bg-base-300"
      }
    >
      <MapLibreRouteMapClient {...props} />
    </div>
  );
}
