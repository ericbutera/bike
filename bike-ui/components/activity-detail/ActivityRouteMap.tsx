"use client";

import { useMemo, useState } from "react";
import { buildActivityClimbs } from "../../lib/activityClimbs";
import { CLIMB_GRADIENT_COLORS } from "../routeMapClimbs";
import type { RouteMapOverlayLayers } from "../RouteMapTypes";
import { FLAG_ENHANCED_MAPS, useFeatureFlag } from "../../lib/featureFlags";
import {
  DEFAULT_ROUTE_BASEMAP,
  DEFAULT_ROUTE_COLOR,
  DEFAULT_ROUTE_LINE_WIDTH,
  DEFAULT_ROUTE_CASING_WIDTH,
} from "../../lib/mapAppearance";
import type { Activity } from "./types";
import MapLibreRouteMap from "../MapLibreRouteMap";
import { AppCard, CardHeader } from "../ui/Card";
import ActivitySegmentLegend from "./ActivitySegmentLegend";
import {
  segmentOverlayPoints,
  useMatchedSegmentGroups,
} from "./matchedSegments";

type SegmentHoverSource = "map" | "legend";

export default function ActivityRouteMap({
  activity,
  onSelectSegment,
  onClearSegmentSelection,
  selectedSegmentIds,
}: {
  activity: Activity;
  onSelectSegment: (segmentId: number) => void;
  onClearSegmentSelection: () => void;
  selectedSegmentIds: number[];
}) {
  const routePoints = activity.route_points;
  const showNewActivityMaps = useFeatureFlag(FLAG_ENHANCED_MAPS);
  const segmentGroups = useMatchedSegmentGroups(activity);
  const climbs = useMemo(() => buildActivityClimbs(routePoints), [routePoints]);
  const [visibleLayers, setVisibleLayers] = useState({
    segments: true,
    climbs: false,
  });
  const overlayLayers: RouteMapOverlayLayers = {
    ...visibleLayers,
    onChange: (layer, visible) => {
      setVisibleLayers((current) => ({ ...current, [layer]: visible }));
      if (layer === "segments") setHoveredSegment(null);
    },
  };
  const climbOverlays = visibleLayers.climbs
    ? climbs.map((climb) => ({
        id: climb.id,
        label: `Climb ${climb.sequence}`,
        points: climb.routePoints,
      }))
    : [];
  const [hoveredSegment, setHoveredSegment] = useState<{
    segmentId: number;
    source: SegmentHoverSource;
  } | null>(null);
  function setSegmentHover(
    segmentId: number,
    source: SegmentHoverSource,
    isHovered: boolean,
  ) {
    setHoveredSegment((current) => {
      if (current?.source === "legend" && source === "map") return current;
      const isCurrentHover =
        current?.segmentId === segmentId && current.source === source;
      if (!isHovered) return isCurrentHover ? null : current;
      return isCurrentHover ? current : { segmentId, source };
    });
  }
  const focusedSegmentIds =
    hoveredSegment == null ? selectedSegmentIds : [hoveredSegment.segmentId];
  const hasRouteMap = (routePoints?.length ?? 0) >= 2;
  const segmentOverlays = segmentGroups
    .flatMap((segmentGroup) =>
      segmentGroup.efforts.map((segmentEffort) => ({
        id: `${segmentEffort.segment_id}-${segmentEffort.effort_index}`,
        color:
          focusedSegmentIds.length === 0 ||
          focusedSegmentIds.includes(segmentGroup.segmentId)
            ? segmentGroup.tone.mapColor
            : DEFAULT_ROUTE_COLOR,
        label: segmentGroup.segmentTitle,
        points: segmentOverlayPoints(routePoints, segmentEffort),
        weight: DEFAULT_ROUTE_LINE_WIDTH,
        casingColor: "#ffffff",
        casingWeight: DEFAULT_ROUTE_CASING_WIDTH,
        focused: focusedSegmentIds.includes(segmentGroup.segmentId),
        onClick: () => {
          onSelectSegment(segmentGroup.segmentId);
        },
        onMouseEnter: () =>
          setSegmentHover(segmentGroup.segmentId, "map", true),
        onMouseLeave: () =>
          setSegmentHover(segmentGroup.segmentId, "map", false),
      })),
    )
    .filter((overlay) => overlay.points.length >= 2)
    .sort((left, right) => Number(left.focused) - Number(right.focused));

  return (
    <AppCard>
      <CardHeader className="mb-3" title="Route map" />

      {hasRouteMap ? (
        <>
          <div className="overflow-hidden border border-base-300 bg-base-200">
            <MapLibreRouteMap
              routePoints={routePoints}
              overlays={visibleLayers.segments ? segmentOverlays : []}
              climbOverlays={climbOverlays}
              overlayLayers={overlayLayers}
              ariaLabel="Activity route map"
              emptyMessage="This activity does not have enough stored route points for the map yet."
              showZoomControls
              showLayerPicker={!showNewActivityMaps}
              defaultBasemap={DEFAULT_ROUTE_BASEMAP}
              className="h-96 w-full"
            />
          </div>

          {visibleLayers.climbs ? (
            <div className="mt-3 flex items-center gap-2 text-xs text-base-content/70">
              <span
                aria-hidden="true"
                className="h-1.5 w-20 rounded-full"
                style={{
                  background: `linear-gradient(to right, ${CLIMB_GRADIENT_COLORS.join(",")})`,
                }}
              />
              <span>Climb start → finish</span>
            </div>
          ) : null}
          {visibleLayers.segments && segmentGroups.length > 0 ? (
            <ActivitySegmentLegend
              segmentGroups={segmentGroups}
              selectedSegmentIds={selectedSegmentIds}
              onSelectSegment={onSelectSegment}
              onHoverSegment={(segmentId, isHovered) =>
                setSegmentHover(segmentId, "legend", isHovered)
              }
              onClearSelection={() => {
                setHoveredSegment(null);
                onClearSegmentSelection();
              }}
            />
          ) : null}
        </>
      ) : (
        <div className="alert mt-5">
          <div>
            <p>
              This activity does not have enough stored route points for the map
              yet. Regenerate it once to rebuild the full route geometry and
              re-run segment matching.
            </p>
          </div>
        </div>
      )}
    </AppCard>
  );
}
