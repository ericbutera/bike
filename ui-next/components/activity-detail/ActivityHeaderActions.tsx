"use client";

import { faBars } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import {
  ACTIVITY_TYPE_OPTIONS,
  formatActivityTypeLabel,
  normalizeActivityType,
  type ActivityType,
} from "../../lib/activityTypes";
import { formatSport } from "../../lib/activityFormatting";
import type { Activity } from "../../lib/queries";
import { hasSegmentBuilderRoute } from "../../lib/segmentBuilder";

export function ActivityHeaderActions({
  activity,
  isRegenerating,
  isDeleting,
  onOpenActivityTypeDialog,
  onRegenerate,
  onDelete,
}: {
  activity: Activity;
  isRegenerating: boolean;
  isDeleting: boolean;
  onOpenActivityTypeDialog: () => void;
  onRegenerate: () => void;
  onDelete: () => void;
}) {
  const canBuildSegment = hasSegmentBuilderRoute(activity.route_points);
  const segmentBuilderHref = `/segments/builder?activityId=${activity.id}`;

  return (
    <div className="flex flex-col items-start gap-3 sm:items-end">
      <div className="flex flex-wrap gap-2">
        <span className="badge badge-outline">{formatSport(activity.sport)}</span>
        <span className="badge badge-outline">
          {formatActivityTypeLabel(activity.activity_type)}
        </span>

        <div className="dropdown dropdown-end">
          <button
            type="button"
            tabIndex={0}
            className="btn btn-ghost btn-square btn-sm"
            aria-label="Open activity actions"
          >
            <FontAwesomeIcon icon={faBars} className="h-4 w-4" />
          </button>
          <ul
            tabIndex={0}
            className="dropdown-content menu z-20 mt-2 w-56 rounded-box border border-base-300 bg-base-100 p-2 shadow-lg"
          >
            <li>
              {canBuildSegment && (
                <Link href={segmentBuilderHref}>Build segment</Link>
              )}
            </li>
            <li>
              <button type="button" onClick={onOpenActivityTypeDialog}>
                Activity type
              </button>
            </li>

            {activity.can_regenerate ? (
              <li>
                <button
                  type="button"
                  onClick={onRegenerate}
                  disabled={isRegenerating}
                >
                  {isRegenerating ? "Regenerating..." : "Regenerate derived data"}
                </button>
              </li>
            ) : null}
            <li>
              <button
                type="button"
                className="text-error"
                onClick={onDelete}
                disabled={isDeleting}
              >
                {isDeleting ? "Deleting..." : "Delete activity"}
              </button>
            </li>
          </ul>
        </div>
      </div>

      {!canBuildSegment ? (
        <p className="text-sm text-base-content/60 sm:text-right">
          This ride needs stored route points before Bike can build a segment
          from it.
        </p>
      ) : null}
    </div>
  );
}

export function ActivityTypeDialog({
  activityTypeDraft,
  isSaving,
  onCancel,
  onSave,
  onChange,
}: {
  activityTypeDraft: ActivityType;
  isSaving: boolean;
  onCancel: () => void;
  onSave: () => void;
  onChange: (activityType: ActivityType) => void;
}) {
  return (
    <div className="modal modal-open">
      <div className="modal-box max-w-md">
        <h2 className="text-xl font-semibold text-base-content">
          Activity type
        </h2>
        <div className="mt-5 space-y-3">
          {ACTIVITY_TYPE_OPTIONS.map((option) => (
            <label
              key={option.value}
              className={`flex cursor-pointer items-start gap-3 rounded-box border p-4 ${
                activityTypeDraft === option.value
                  ? "border-primary bg-primary/10"
                  : "border-base-300 bg-base-100"
              }`}
            >
              <input
                type="radio"
                name="activity-type"
                className="radio radio-primary mt-1"
                value={option.value}
                checked={activityTypeDraft === option.value}
                onChange={(event) =>
                  onChange(normalizeActivityType(event.target.value))
                }
              />
              <span>
                <span className="block font-medium text-base-content">
                  {option.label}
                </span>
                <span className="mt-1 block text-sm leading-6 text-base-content/65">
                  {option.description}
                </span>
              </span>
            </label>
          ))}
        </div>
        <div className="modal-action">
          <button
            type="button"
            className="btn btn-ghost"
            disabled={isSaving}
            onClick={onCancel}
          >
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={isSaving}
            onClick={onSave}
          >
            {isSaving ? "Saving..." : "Save type"}
          </button>
        </div>
      </div>
      <button
        type="button"
        className="modal-backdrop"
        aria-label="Close activity type dialog"
        disabled={isSaving}
        onClick={onCancel}
      />
    </div>
  );
}
