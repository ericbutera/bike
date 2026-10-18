import { faRectangleXmark } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import type { MatchedSegmentGroup } from "./matchedSegments";
import SegmentColorStatus from "./SegmentColorStatus";

export default function ActivitySegmentLegend({
  segmentGroups,
  selectedSegmentIds,
  onSelectSegment,
  onHoverSegment,
  onClearSelection,
}: {
  segmentGroups: MatchedSegmentGroup[];
  selectedSegmentIds: number[];
  onSelectSegment: (segmentId: number) => void;
  onHoverSegment: (segmentId: number, isHovered: boolean) => void;
  onClearSelection: () => void;
}) {
  return (
    <div
      className="card-actions mt-4 gap-2"
      role="group"
      aria-label="Map segments"
    >
      {segmentGroups.map((segmentGroup) => {
        const isSelected = selectedSegmentIds.includes(segmentGroup.segmentId);

        return (
          <button
            key={segmentGroup.segmentId}
            type="button"
            className={`btn btn-sm ${isSelected ? "btn-active" : "btn-outline"}`}
            aria-label={`Highlight ${segmentGroup.segmentTitle}`}
            aria-pressed={isSelected}
            onMouseEnter={() => onHoverSegment(segmentGroup.segmentId, true)}
            onMouseLeave={() => onHoverSegment(segmentGroup.segmentId, false)}
            onFocus={() => onHoverSegment(segmentGroup.segmentId, true)}
            onBlur={() => onHoverSegment(segmentGroup.segmentId, false)}
            onClick={() => onSelectSegment(segmentGroup.segmentId)}
          >
            <SegmentColorStatus color={segmentGroup.tone.mapColor} />
            <span>{segmentGroup.segmentTitle}</span>
            <span className="badge badge-ghost badge-sm">
              {segmentGroup.efforts.length}
            </span>
          </button>
        );
      })}
      {selectedSegmentIds.length > 0 ? (
        <button
          type="button"
          className="btn btn-sm"
          aria-label={`Clear selection (${selectedSegmentIds.length})`}
          title={`Clear selection (${selectedSegmentIds.length})`}
          onClick={onClearSelection}
        >
          <FontAwesomeIcon
            icon={faRectangleXmark}
            style={{ height: "1.2em", width: "1.2em" }}
          />
          Clear selection
        </button>
      ) : null}
    </div>
  );
}
