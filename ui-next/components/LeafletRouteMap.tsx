import dynamic from "next/dynamic";
import { type LeafletRouteMapProps } from "./LeafletRouteMapClient";

const LeafletRouteMapClient = dynamic(
  () => import("./LeafletRouteMapClient").then((module) => module.default),
  {
    ssr: false,
    loading: () => null,
  },
);

export default function LeafletRouteMap(props: LeafletRouteMapProps) {
  const hasRoute = (props.routePoints?.length ?? 0) >= 2;

  if (!hasRoute) {
    return <div className="alert">{props.emptyMessage}</div>;
  }

  return (
    <div
      role="img"
      aria-label={props.ariaLabel}
      className={
        props.className ??
        "h-96 w-full overflow-hidden rounded-box border border-base-300 bg-base-300"
      }
    >
      <LeafletRouteMapClient {...props} />
    </div>
  );
}
