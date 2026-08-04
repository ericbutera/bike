"use client";

import { useMemo } from "react";
import { buildActivityClimbs } from "../../lib/activityClimbs";
import { useActivity } from "../../lib/queries";
import MapLibreRouteMap from "../MapLibreRouteMap";
import {
  segmentOverlayPoints,
  useMatchedSegmentGroups,
} from "./matchedSegments";

export default function ActivityRouteMap({
  activityId,
  onSelectSegment,
  onSelectClimb,
  selectedSegmentId,
  selectedClimbId,
}: {
  activityId: number | string;
  onSelectSegment: (segmentId: number) => void;
  onSelectClimb: (climbId: string) => void;
  selectedSegmentId: number | null;
  selectedClimbId: string | null;
}) {
  const activityQuery = useActivity(activityId);
  const activity = activityQuery.data;
  const routePoints = activity?.route_points;
  const segmentGroups = useMatchedSegmentGroups(activity);
  const climbs = useMemo(
    () => buildActivityClimbs(routePoints),
    [routePoints],
  );
  const hasRouteMap = (routePoints?.length ?? 0) >= 2;
  const selectedClimb =
    climbs.find((climb) => climb.id === selectedClimbId) ?? null;
  const segmentOverlays = segmentGroups
    .flatMap((segmentGroup) =>
      segmentGroup.efforts.map((segmentEffort) => ({
        id: `${segmentEffort.segment_id}-${segmentEffort.effort_index}`,
        color: segmentGroup.tone.mapColor,
        label: segmentGroup.segmentTitle,
        points: segmentOverlayPoints(routePoints, segmentEffort),
        weight: selectedSegmentId === segmentGroup.segmentId ? 8 : 6,
        onClick: () => {
          onSelectSegment(segmentGroup.segmentId);
        },
      })),
    )
    .filter((overlay) => overlay.points.length >= 2);
  const climbOverlays = climbs
    .map((climb) => {
      const isSelected = selectedClimbId === climb.id;

      return {
        id: climb.id,
        color: isSelected ? "#f97316" : "#f59e0b",
        label: `Climb ${climb.sequence}`,
        points: climb.routePoints,
        weight: isSelected ? 9 : 5,
        onClick: () => {
          onSelectClimb(climb.id);
        },
      };
    })
    .filter((overlay) => overlay.points.length >= 2);
  const overlays = [...segmentOverlays, ...climbOverlays];

  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body">
        {hasRouteMap ? (
          <>
            <div className="mt-5 overflow-hidden border border-base-300 bg-base-200">
              <MapLibreRouteMap
                routePoints={routePoints}
                overlays={overlays}
                ariaLabel="Activity route map"
                emptyMessage="This activity does not have enough stored route points for the map yet."
                showZoomControls
                showLayerPicker
                defaultBasemap="topo"
                basemapOptions={["topo", "street", "satellite"]}
                fitBoundsPoints={selectedClimb?.routePoints ?? null}
                fitBoundsKey={selectedClimb ? selectedClimb.id : "activity"}
                fitBoundsMaxZoom={selectedClimb ? 15 : undefined}
                className="h-96 w-full"
              />
            </div>

            {segmentGroups.length > 0 ? (
              <div className="card-actions mt-4 gap-2">
                {segmentGroups.map((segmentGroup) => {
                  const isSelected =
                    selectedSegmentId === segmentGroup.segmentId;

                  return (
                    <button
                      key={`${segmentGroup.segmentId}-legend`}
                      type="button"
                      className={`btn btn-sm ${isSelected ? segmentGroup.tone.buttonClassName : segmentGroup.tone.outlineButtonClassName}`}
                      aria-label={`Jump to ${segmentGroup.segmentTitle} matches`}
                      onClick={() => {
                        onSelectSegment(segmentGroup.segmentId);
                      }}
                    >
                      <span
                        aria-hidden
                        className={`inline-block h-2.5 w-2.5 rounded-full ${segmentGroup.tone.dotClassName}`}
                      />
                      <span>{segmentGroup.segmentTitle}</span>
                      <span className="badge badge-ghost badge-sm">
                        {segmentGroup.efforts.length}
                      </span>
                    </button>
                  );
                })}
              </div>
            ) : null}
          </>
        ) : (
          <div className="alert mt-5">
            <div>
              <p>
                This activity does not have enough stored route points for the
                map yet. Regenerate it once to rebuild the full route geometry
                and re-run segment matching.
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
