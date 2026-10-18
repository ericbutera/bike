"use client";

import { useState } from "react";
import { faRoute } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { DEFAULT_ROUTE_COLOR } from "../lib/mapAppearance";
import type { ActivityRoutePoint } from "../lib/queries";
import MapLibreRouteMap from "./MapLibreRouteMap";
import Modal from "./ui/Modal";

export default function ViewRouteOnMap({
  title,
  routePoints,
  color = DEFAULT_ROUTE_COLOR,
}: {
  title: string;
  routePoints: ActivityRoutePoint[];
  color?: string;
}) {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <>
      <button
        type="button"
        className="btn btn-ghost btn-sm btn-square"
        aria-label={`View ${title} on map`}
        title={`View ${title} on map`}
        disabled={routePoints.length < 2}
        onKeyDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          event.stopPropagation();
          setIsOpen(true);
        }}
      >
        <FontAwesomeIcon icon={faRoute} className="h-4 w-4" />
      </button>
      {isOpen ? (
        <Modal
          title={title}
          onClose={() => setIsOpen(false)}
          className="max-w-5xl"
        >
          <MapLibreRouteMap
            routePoints={routePoints}
            routeColor={color}
            ariaLabel={`${title} map`}
            emptyMessage="This route section does not have enough stored points for a map."
            showZoomControls
            fitBoundsMaxZoom={16}
            className="mt-4 h-[min(60vh,32rem)] w-full overflow-hidden rounded-box border border-base-300"
          />
        </Modal>
      ) : null}
    </>
  );
}
