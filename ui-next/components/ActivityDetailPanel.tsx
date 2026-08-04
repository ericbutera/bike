"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import {
  extractApiMessage,
  formatActivityTimestamp,
} from "../lib/activityFormatting";
import {
  ACTIVITY_TYPES,
  normalizeActivityType,
  type ActivityType,
} from "../lib/activityTypes";
import {
  useActivity,
  useDeleteActivity,
  useRegenerateActivity,
  useUpdateActivity,
} from "../lib/queries";
import { useUnitPreferences } from "../lib/unitPreferences";
import ActivityClimbsCard from "./activity-detail/ActivityClimbsCard";
import {
  ActivityHeaderActions,
  ActivityTypeDialog,
} from "./activity-detail/ActivityHeaderActions";
import LapCard from "./activity-detail/LapCard";
import ActivityMetricsSummary from "./activity-detail/ActivityMetricsSummary";
import ActivityRouteMap from "./activity-detail/ActivityRouteMap";
import ActivitySignalsCard from "./activity-detail/ActivitySignalsCard";
import { buildSegmentAnchorId } from "./activity-detail/matchedSegments";
import MatchedSegmentsSection from "./MatchedSegmentsSection";
import TrainingProfileSnapshot from "./TrainingProfileSnapshot";

export default function ActivityDetailPanel({
  activityId,
}: {
  activityId: number | string;
}) {
  const [selectedSegmentId, setSelectedSegmentId] = useState<number | null>(
    null,
  );
  const [selectedClimbId, setSelectedClimbId] = useState<string | null>(null);
  const [isActivityTypeDialogOpen, setIsActivityTypeDialogOpen] =
    useState(false);
  const [activityTypeDraft, setActivityTypeDraft] = useState<ActivityType>(
    ACTIVITY_TYPES.Training,
  );
  const router = useRouter();
  const { unitSystem } = useUnitPreferences();
  const activityQuery = useActivity(activityId);
  const regenerateMutation = useRegenerateActivity();
  const deleteMutation = useDeleteActivity();
  const updateActivityMutation = useUpdateActivity();
  const activity = activityQuery.data;

  useEffect(() => {
    setActivityTypeDraft(normalizeActivityType(activity?.activity_type));
  }, [activity?.activity_type]);

  function focusSegmentMatch(segmentId: number) {
    setSelectedSegmentId(segmentId);

    if (typeof document === "undefined") {
      return;
    }

    const matchCard = document.getElementById(buildSegmentAnchorId(segmentId));

    if (!(matchCard instanceof HTMLElement)) {
      return;
    }

    matchCard.scrollIntoView({ behavior: "smooth", block: "start" });

    const firstLink = matchCard.querySelector("a");

    if (firstLink instanceof HTMLElement) {
      firstLink.focus({ preventScroll: true });
    }
  }

  function focusClimb(climbId: string) {
    setSelectedClimbId(climbId);

    if (typeof document === "undefined") {
      return;
    }

    const climbsCard = document.getElementById("activity-climbs-card");

    if (
      climbsCard instanceof HTMLElement &&
      typeof climbsCard.scrollIntoView === "function"
    ) {
      climbsCard.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }

  function toggleSegmentMatch(segmentId: number) {
    setSelectedSegmentId((current) => (current === segmentId ? null : current));
  }

  async function handleRegenerate() {
    if (!activity) {
      return;
    }

    try {
      await regenerateMutation.regenerateAsync(activity.id);
    } catch {
      // The mutation exposes the API error state used below.
    }
  }

  async function handleSaveActivityType() {
    if (!activity) {
      return;
    }

    try {
      await updateActivityMutation.updateAsync(activity.id, {
        activity_type: activityTypeDraft,
      });
      setIsActivityTypeDialogOpen(false);
    } catch {
      // The mutation exposes the API error state used below.
    }
  }

  async function handleDelete() {
    if (!activity) {
      return;
    }

    const confirmed =
      typeof globalThis.confirm === "function"
        ? globalThis.confirm(
            "Delete this activity? This removes the activity and clears any derived segment matches.",
          )
        : true;

    if (!confirmed) {
      return;
    }

    try {
      await deleteMutation.deleteAsync(activity.id);
      router.push("/");
    } catch {
      // The mutation exposes the API error state used below.
    }
  }

  if (activityQuery.isLoading) {
    return (
      <section className="card bg-base-100 shadow-xl">
        <div className="card-body items-center py-10">
          <span className="loading loading-spinner loading-md" />
        </div>
      </section>
    );
  }

  if (activityQuery.isError || !activity) {
    return (
      <section className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="alert alert-error">
            {extractApiMessage(activityQuery.error) || "Activity not found"}
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="space-y-8">
      <div className="card bg-base-100 shadow-xl">
        <div className="card-body gap-6">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <h1 className="mt-2 text-4xl font-semibold">{activity.title}</h1>
              <p className="mt-3 text-sm text-base-content/70">
                {formatActivityTimestamp(activity.started_at)}
                {" - "}
                {activity.location}
              </p>
            </div>
            <ActivityHeaderActions
              activity={activity}
              isRegenerating={regenerateMutation.isPending}
              isDeleting={deleteMutation.isPending}
              onOpenActivityTypeDialog={() => {
                setActivityTypeDraft(
                  normalizeActivityType(activity.activity_type),
                );
                setIsActivityTypeDialogOpen(true);
              }}
              onRegenerate={() => {
                void handleRegenerate();
              }}
              onDelete={() => {
                void handleDelete();
              }}
            />
          </div>

          {regenerateMutation.isError ? (
            <div className="alert alert-error">
              {extractApiMessage(regenerateMutation.error)}
            </div>
          ) : null}

          {deleteMutation.isError ? (
            <div className="alert alert-error">
              {extractApiMessage(deleteMutation.error)}
            </div>
          ) : null}

          {updateActivityMutation.isError ? (
            <div className="alert alert-error">
              {extractApiMessage(updateActivityMutation.error)}
            </div>
          ) : null}

          {isActivityTypeDialogOpen ? (
            <ActivityTypeDialog
              activityTypeDraft={activityTypeDraft}
              isSaving={updateActivityMutation.isPending}
              onCancel={() => setIsActivityTypeDialogOpen(false)}
              onSave={() => {
                void handleSaveActivityType();
              }}
              onChange={setActivityTypeDraft}
            />
          ) : null}

          <ActivityMetricsSummary activity={activity} unitSystem={unitSystem} />
        </div>
      </div>

      <ActivityRouteMap
        activityId={activityId}
        onSelectSegment={focusSegmentMatch}
        onSelectClimb={focusClimb}
        selectedSegmentId={selectedSegmentId}
        selectedClimbId={selectedClimbId}
      />

      <ActivityClimbsCard
        activityId={activityId}
        selectedClimbId={selectedClimbId}
        unitSystem={unitSystem}
        onSelectClimb={focusClimb}
        onZoomOutMap={() => setSelectedClimbId(null)}
      />

      <MatchedSegmentsSection
        activityId={activityId}
        selectedSegmentId={selectedSegmentId}
        onToggleSegmentMatch={toggleSegmentMatch}
      />

      <TrainingProfileSnapshot
        estimatedFtpWatts={activity.estimated_ftp_watts}
        heartRateZones={activity.heart_rate_zones}
      />

      <ActivitySignalsCard
        chartPoints={activity.chart_points}
        unitSystem={unitSystem}
      />

      <div className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <h2 className="card-title text-xl">Lap splits</h2>
              <p className="text-sm text-base-content/70">
                These lap rollups come from the upload-time read side and can be
                regenerated when the development flag is enabled.
              </p>
            </div>
            <span className="badge badge-outline">
              {(activity.laps ?? []).length} lap
              {(activity.laps ?? []).length === 1 ? "" : "s"}
            </span>
          </div>

          {(activity.laps ?? []).length > 0 ? (
            <div className="mt-5 grid gap-4 xl:grid-cols-2">
              {(activity.laps ?? []).map((lap) => (
                <LapCard
                  key={`${lap.lap_index}-${lap.title}`}
                  lap={lap}
                  unitSystem={unitSystem}
                />
              ))}
            </div>
          ) : (
            <div className="alert mt-5">
              <span>This upload did not contain explicit lap data.</span>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
