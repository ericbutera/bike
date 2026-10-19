"use client";

import { faClock, faLocationDot } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { formatActivityTimestamp } from "../lib/activityFormatting";
import { useActivity, useDeleteActivity } from "../lib/queries";
import { useUnitPreferences } from "../lib/unitPreferences";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import ActivityRouteDetails from "./activity-detail/ActivityRouteDetails";
import { ActivityHeaderActions } from "./activity-detail/ActivityHeaderActions";
import ActivityModal from "./activity-detail/ActivityModal";
import ActivityDataCard from "./activity-detail/ActivityDataCard";
import ActivityLapsTable from "./activity-detail/ActivityLapsTable";
import ActivityMetricsSummary from "./activity-detail/ActivityMetricsSummary";
import ActivityRouteMap from "./activity-detail/ActivityRouteMap";
import ActivitySignalsCard from "./activity-detail/ActivitySignalsCard";
import { useKeyedState } from "../lib/useKeyedState";
import TrainingProfileSnapshot from "./TrainingProfileSnapshot";
import { AppCard, CardHeader } from "./ui/Card";
import InfoTooltip from "./ui/InfoTooltip";
import { ErrorCard, LoadingCard } from "./ui/QueryState";

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
  const activityQuery = useActivity(activityId);
  useDocumentTitle(activityQuery.data?.title ?? "Activity");
  const deleteMutation = useDeleteActivity();
  const activity = activityQuery.data;
  function toggleSegmentMatch(segmentId: number) {
    setSelectedSegmentIds((current) =>
      current.includes(segmentId)
        ? current.filter((id) => id !== segmentId)
        : [...current, segmentId],
    );
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
    <section className="space-y-4">
      <header className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 className="text-3xl font-semibold text-base-content">
            {activity.title}
          </h1>
          <p className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-base-content/70">
            {activity.location ? (
              <span className="inline-flex items-center gap-1.5">
                <FontAwesomeIcon icon={faLocationDot} aria-hidden="true" />
                {activity.location}
              </span>
            ) : null}
            <time
              dateTime={activity.started_at}
              className="inline-flex items-center gap-1.5"
            >
              <FontAwesomeIcon icon={faClock} aria-hidden="true" />
              {formatActivityTimestamp(activity.started_at)}
            </time>
          </p>
        </div>
        <ActivityHeaderActions
          activity={activity}
          isDeleting={deleteMutation.isPending}
          onOpenEditDialog={() => setIsActivityModalOpen(true)}
          onDelete={() => {
            void handleDelete();
          }}
        />
      </header>

      {isActivityModalOpen ? (
        <ActivityModal
          activity={activity}
          onClose={() => setIsActivityModalOpen(false)}
        />
      ) : null}

      <ActivityMetricsSummary activity={activity} unitSystem={unitSystem} />
      <ActivityDataCard activity={activity} unitSystem={unitSystem} />

      <ActivityRouteMap
        key={activity.id}
        activity={activity}
        onSelectSegment={toggleSegmentMatch}
        onClearSegmentSelection={() => setSelectedSegmentIds([])}
        selectedSegmentIds={selectedSegmentIds}
      />

      <ActivityRouteDetails
        key={`details-${activity.id}`}
        activity={activity}
        unitSystem={unitSystem}
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

      <AppCard as="section" aria-label="Lap splits">
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
            <span className="text-sm text-base-content/60">
              {(activity.laps ?? []).length} lap
              {(activity.laps ?? []).length === 1 ? "" : "s"}
            </span>
          }
        />

        {(activity.laps ?? []).length > 0 ? (
          <ActivityLapsTable
            laps={activity.laps ?? []}
            unitSystem={unitSystem}
          />
        ) : (
          <div className="alert mt-5">
            <span>This upload did not contain explicit lap data.</span>
          </div>
        )}
      </AppCard>
    </section>
  );
}
