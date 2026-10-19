"use client";

import { faBars } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { activitySourceFileUrl } from "../../lib/activitySourceFiles";
import {
  useUpdateActivity,
  type Activity,
  type UpdateActivityInput,
} from "../../lib/queries";
import { hasSegmentBuilderRoute } from "../../lib/segmentBuilder";
import {
  ActivitySportSelect,
  ActivityTypeSelect,
} from "./ActivityClassificationSelect";

export function ActivityHeaderActions({
  activity,
  isDeleting,
  onOpenEditDialog,
  onDelete,
}: {
  activity: Activity;
  isDeleting: boolean;
  onOpenEditDialog: () => void;
  onDelete: () => void;
}) {
  const canBuildSegment = hasSegmentBuilderRoute(activity.route_points);
  const segmentBuilderHref = `/segments/builder?activityId=${activity.id}`;
  const update = useUpdateActivity();
  async function saveClassification(patch: UpdateActivityInput) {
    try {
      await update.updateAsync(activity.id, patch);
    } catch {
      /* The mutation exposes the save error below. */
    }
  }

  return (
    <div className="flex flex-col items-start gap-3 sm:items-end">
      <div className="flex flex-wrap items-center justify-end gap-2">
        <ActivitySportSelect
          value={activity.sport}
          iconOnly
          disabled={update.isPending}
          onChange={(sport) => {
            void saveClassification({ sport });
          }}
        />
        <ActivityTypeSelect
          value={activity.activity_type}
          disabled={update.isPending}
          onChange={(activity_type) => {
            void saveClassification({ activity_type });
          }}
        />

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
              <button type="button" onClick={onOpenEditDialog}>
                Edit activity
              </button>
            </li>
            {canBuildSegment ? (
              <li>
                <Link href={segmentBuilderHref}>Build segment</Link>
              </li>
            ) : null}
            {activity.can_download_source_file ? (
              <li>
                <a href={activitySourceFileUrl(activity.id)}>
                  Download original source
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
      {update.isError ? (
        <p role="alert" className="text-sm text-error">
          Unable to update activity. Please try again.
        </p>
      ) : null}

      {!canBuildSegment ? (
        <p className="text-sm text-base-content/60 sm:text-right">
          This ride needs stored route points before Bike can build a segment
          from it.
        </p>
      ) : null}
    </div>
  );
}
