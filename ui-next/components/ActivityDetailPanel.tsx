"use client";

import { auth } from "@ericbutera/kaleido";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { formatActivityTimestamp } from "../lib/activityFormatting";
import {
  useActivity,
  useAdminActivityImportTrace,
  useDeleteActivity,
  useRegenerateActivity,
} from "../lib/queries";
import { useUnitPreferences } from "../lib/unitPreferences";
import ActivityClimbsCard from "./activity-detail/ActivityClimbsCard";
import { ActivityHeaderActions } from "./activity-detail/ActivityHeaderActions";
import ActivityImportTracePanel from "./activity-detail/ActivityImportTracePanel";
import ActivityModal from "./activity-detail/ActivityModal";
import LapCard from "./activity-detail/LapCard";
import ActivityMetricsSummary from "./activity-detail/ActivityMetricsSummary";
import ActivityRouteMap from "./activity-detail/ActivityRouteMap";
import ActivitySignalsCard from "./activity-detail/ActivitySignalsCard";
import { buildSegmentAnchorId } from "./activity-detail/matchedSegments";
import MatchedSegmentsSection from "./MatchedSegmentsSection";
import TrainingProfileSnapshot from "./TrainingProfileSnapshot";
import { AppCard, CardHeader } from "./ui/Card";
import InfoTooltip from "./ui/InfoTooltip";

const LAP_SPLITS_HELP_TEXT =
  "These lap rollups come from the upload-time read side and can be regenerated when the development flag is enabled.";

export default function ActivityDetailPanel({
  activityId,
}: {
  activityId: number | string;
}) {
  const [selectedSegmentId, setSelectedSegmentId] = useState<number | null>(
    null,
  );
  const [selectedClimbId, setSelectedClimbId] = useState<string | null>(null);
  const [isActivityModalOpen, setIsActivityModalOpen] = useState(false);
  const [isImportTraceModalOpen, setIsImportTraceModalOpen] = useState(false);
  const router = useRouter();
  const { unitSystem } = useUnitPreferences();
  const authApi = auth.useAuthApi();
  const { user } = authApi.useCurrentUser();
  const activityQuery = useActivity(activityId);
  const regenerateMutation = useRegenerateActivity();
  const deleteMutation = useDeleteActivity();
  const activity = activityQuery.data;
  const isAdmin = Boolean(user?.is_admin);
  const importTraceQuery = useAdminActivityImportTrace(activity?.id ?? null, {
    enabled: isAdmin && !!activity,
  });

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

  if (!activity) {
    return null;
  }

  return (
    <section className="space-y-8">
      <AppCard bodyClassName="gap-6">
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
            canViewImportTrace={isAdmin && !!importTraceQuery.data}
            onOpenEditDialog={() => setIsActivityModalOpen(true)}
            onViewImportTrace={() => setIsImportTraceModalOpen(true)}
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

        {isImportTraceModalOpen ? (
          <ActivityImportTraceModal
            trace={importTraceQuery.data}
            isLoading={importTraceQuery.isLoading}
            error={importTraceQuery.error}
            onClose={() => setIsImportTraceModalOpen(false)}
          />
        ) : null}

        <ActivityMetricsSummary activity={activity} unitSystem={unitSystem} />
      </AppCard>

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

function ActivityImportTraceModal({
  trace,
  isLoading,
  error,
  onClose,
}: {
  trace: ReturnType<typeof useAdminActivityImportTrace>["data"];
  isLoading: boolean;
  error: Error | null;
  onClose: () => void;
}) {
  return (
    <dialog className="modal modal-open">
      <div className="modal-box max-w-5xl">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="text-lg font-semibold">Import trace</h2>
            {trace ? (
              <p className="mt-1 text-sm text-base-content/65">
                Import #{trace.import.id}
                {trace.import.import_version
                  ? ` · version ${trace.import.import_version}`
                  : ""}
              </p>
            ) : null}
          </div>
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            onClick={onClose}
          >
            Close
          </button>
        </div>

        <div className="mt-6">
          <ActivityImportTracePanel
            trace={trace}
            isLoading={isLoading}
            error={error}
          />
        </div>
      </div>
      <form method="dialog" className="modal-backdrop">
        <button type="button" onClick={onClose}>
          close
        </button>
      </form>
    </dialog>
  );
}
