"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import TemporalLink from "./ui/TemporalLink";
import { formatActivityTimestamp } from "../lib/activityFormatting";
import {
  useActivity,
  useDeleteActivity,
  useRegenerateActivity,
} from "../lib/queries";
import { useUnitPreferences } from "../lib/unitPreferences";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import ActivityClimbsCard from "./activity-detail/ActivityClimbsCard";
import { ActivityHeaderActions } from "./activity-detail/ActivityHeaderActions";
import ActivityModal from "./activity-detail/ActivityModal";
import LapCard from "./activity-detail/LapCard";
import ActivityMetricsSummary from "./activity-detail/ActivityMetricsSummary";
import ActivityRouteMap from "./activity-detail/ActivityRouteMap";
import ActivitySignalsCard from "./activity-detail/ActivitySignalsCard";
import { useKeyedState } from "../lib/useKeyedState";
import MatchedSegmentsSection from "./MatchedSegmentsSection";
import TrainingProfileSnapshot from "./TrainingProfileSnapshot";
import { AppCard, CardHeader } from "./ui/Card";
import InfoTooltip from "./ui/InfoTooltip";
import { ErrorCard, LoadingCard } from "./ui/QueryState";
import { useAuth } from "../lib/auth";

const LAP_SPLITS_HELP_TEXT =
  "These lap rollups come from the upload-time read side and can be regenerated when the development flag is enabled.";

export default function ActivityDetailPanel({
  activityId,
}: {
  activityId: number | string;
}) {
  const [selectedSegmentIds, setSelectedSegmentIds] = useKeyedState<number[]>(
    String(activityId),
    [],
  );
  const [isActivityModalOpen, setIsActivityModalOpen] = useState(false);
  const router = useRouter();
  const { unitSystem } = useUnitPreferences();
  const { user } = useAuth();
  const activityQuery = useActivity(activityId);
  useDocumentTitle(activityQuery.data?.title ?? "Activity");
  const regenerateMutation = useRegenerateActivity();
  const [reprocessingJob, setReprocessingJob] = useState<string | null>(null);
  const deleteMutation = useDeleteActivity();
  const activity = activityQuery.data;
  const isAdmin = Boolean(user?.is_admin);
  function toggleSegmentMatch(segmentId: number) {
    setSelectedSegmentIds((current) =>
      current.includes(segmentId)
        ? current.filter((id) => id !== segmentId)
        : [...current, segmentId],
    );
  }

  async function handleRegenerate() {
    if (!activity) {
      return;
    }

    try {
      const job = await regenerateMutation.regenerateAsync(activity.id);
      setReprocessingJob(job.id);
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
    return <LoadingCard size="lg" />;
  }

  if (activityQuery.isError) {
    return <ErrorCard fallback="Unable to load activity." />;
  }

  if (!activity) {
    return <ErrorCard fallback="Activity not found." />;
  }

  return (
    <section className="space-y-8">
      <AppCard bodyClassName="gap-6">
        {reprocessingJob ? (
          <p role="status">
            Activity reprocessing started.{" "}
            <TemporalLink workflowId={reprocessingJob} />
          </p>
        ) : null}
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
            isAdmin={isAdmin}
            onOpenEditDialog={() => setIsActivityModalOpen(true)}
            onRegenerate={() => {
              void handleRegenerate();
            }}
            onDelete={() => {
              void handleDelete();
            }}
          />
        </div>

        {isActivityModalOpen ? (
          <ActivityModal
            activityId={activity.id}
            initialTitle={activity.title}
            initialActivityType={activity.activity_type}
            onClose={() => setIsActivityModalOpen(false)}
          />
        ) : null}

        <ActivityMetricsSummary activity={activity} unitSystem={unitSystem} />
      </AppCard>

      <ActivityRouteMap
        key={activity.id}
        activity={activity}
        onSelectSegment={toggleSegmentMatch}
        onClearSegmentSelection={() => setSelectedSegmentIds([])}
        selectedSegmentIds={selectedSegmentIds}
      />

      <ActivityClimbsCard activity={activity} unitSystem={unitSystem} />

      <MatchedSegmentsSection
        activityId={activityId}
        selectedSegmentIds={selectedSegmentIds}
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

      <AppCard>
        <CardHeader
          className="mb-3"
          title="Lap splits"
          titleExtras={
            <InfoTooltip
              label="Lap splits details"
              tip={LAP_SPLITS_HELP_TEXT}
            />
          }
          actions={
            <span className="badge badge-outline">
              {(activity.laps ?? []).length} lap
              {(activity.laps ?? []).length === 1 ? "" : "s"}
            </span>
          }
        />

        {(activity.laps ?? []).length > 0 ? (
          <div className="grid gap-4 xl:grid-cols-2">
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
      </AppCard>
    </section>
  );
}
