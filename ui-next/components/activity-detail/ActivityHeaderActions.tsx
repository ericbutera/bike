"use client";

import { faBars } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { activitySourceFileUrl } from "../../lib/activitySourceFiles";
import { formatActivityTypeLabel } from "../../lib/activityTypes";
import { formatSport } from "../../lib/activityFormatting";
import type { Activity } from "../../lib/queries";
import { hasSegmentBuilderRoute } from "../../lib/segmentBuilder";

export function ActivityHeaderActions({
  activity,
  isRegenerating,
  isDeleting,
  onOpenEditDialog,
  onRegenerate,
  onDelete,
}: {
  activity: Activity;
  isRegenerating: boolean;
  isDeleting: boolean;
  onOpenEditDialog: () => void;
  onRegenerate: () => void;
  onDelete: () => void;
}) {
  const canBuildSegment = hasSegmentBuilderRoute(activity.route_points);
  const segmentBuilderHref = `/segments/builder?activityId=${activity.id}`;

  return (
    <div className="flex flex-col items-start gap-3 sm:items-end">
      <div className="flex flex-wrap items-center justify-end gap-2">
        <span className="badge badge-outline h-8">
          {formatSport(activity.sport)}
        </span>
        <span className="badge badge-outline h-8">
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
              <button type="button" onClick={onOpenEditDialog}>
                Edit activity
              </button>
            </li>

            {activity.can_regenerate ? (
              <li>
                <button
                  type="button"
                  onClick={onRegenerate}
                  disabled={isRegenerating}
                >
                  {isRegenerating
                    ? "Regenerating..."
                    : "Regenerate derived data"}
                </button>
              </li>
            ) : null}
            {activity.can_download_source_file ? (
              <li>
                <a href={activitySourceFileUrl(activity.id)}>
                  Download source file
                </a>
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
