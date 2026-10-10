"use client";

import ActivityPipelines from "@/components/admin/ActivityPipelines";
import ActivityImportTracePanel from "@/components/activity-detail/ActivityImportTracePanel";
import {
  type AdminActivity,
  useAdminActivities,
  useActivityImportTrace,
} from "@/lib/queries";
import GenericList, { type Column } from "@/components/ui/GenericList";
import { faGear } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { Suspense, useMemo, useState } from "react";
import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";

const ADMIN_ACTIVITIES_PAGE_SIZE = 25;
const AdminActivitiesGridSchema = {} as never;

type AdminActivitiesGridParams = {
  page?: number | string;
  per_page?: number | string;
};

export default function AdminActivitiesPage() {
  return (
    <Suspense>
      <LocalAdminLayout title="Activities">
        <AdminActivitiesContent />
      </LocalAdminLayout>
    </Suspense>
  );
}

function AdminActivitiesContent() {
  const [selectedActivity, setSelectedActivity] =
    useState<AdminActivity | null>(null);
  const traceQuery = useActivityImportTrace(
    selectedActivity?.activity_import_id ?? null,
    {
      admin: true,
      enabled: !!selectedActivity?.activity_import_id,
    },
  );
  const columns = useMemo<Column<AdminActivity, AdminActivitiesGridParams>[]>(
    () => [
      {
        key: "title",
        header: "Activity",
        className: "min-w-72 whitespace-nowrap",
        render: (activity) => (
          <div className="min-w-0">
            <div className="font-medium text-base-content">
              {activity.title}
            </div>
            <div className="mt-1 text-xs text-base-content/60">
              #{activity.id} · user {activity.user_id}
            </div>
          </div>
        ),
      },
      {
        key: "started_at",
        header: "Started",
        className: "min-w-48 whitespace-nowrap",
        render: (activity) => formatDateTime(activity.started_at),
      },
      {
        key: "source",
        header: "Source",
        className: "min-w-44 whitespace-nowrap",
        render: (activity) => (
          <div className="flex flex-col gap-1">
            <span className="badge badge-outline">{activity.source}</span>
            {activity.format ? (
              <span className="text-xs uppercase text-base-content/55">
                {activity.format}
              </span>
            ) : null}
          </div>
        ),
      },
      {
        key: "activity_import_id",
        header: "Import",
        className: "min-w-32 whitespace-nowrap",
        render: (activity) =>
          activity.activity_import_id ? (
            <div className="flex flex-col gap-1">
              <span className="font-mono text-xs">
                #{activity.activity_import_id}
              </span>
              <span className={importStatusClass(activity.import_status)}>
                {activity.import_status ?? "unknown"}
              </span>
            </div>
          ) : (
            <span className="text-base-content/45">None</span>
          ),
      },
      {
        key: "actions",
        header: "",
        className: "w-12 whitespace-nowrap",
        render: (activity) => (
          <button
            type="button"
            className="btn btn-ghost btn-square btn-sm"
            aria-label={`View processing pipeline for ${activity.title}`}
            title="View processing pipeline, timings and import history"
            onClick={() => {
              setSelectedActivity(activity);
            }}
          >
            <FontAwesomeIcon icon={faGear} className="h-4 w-4" />
          </button>
        ),
      },
    ],
    [],
  );

  const useAdminActivitiesGridQuery = (params: AdminActivitiesGridParams) => {
    const page = positiveNumber(params.page, 1);
    const perPage = positiveNumber(params.per_page, ADMIN_ACTIVITIES_PAGE_SIZE);
    const query = useAdminActivities({ page, perPage });

    return {
      data: query.data ?? [],
      isLoading: query.isLoading,
      raw: {
        metadata: {
          page: query.metadata?.page ?? page,
          per_page: query.metadata?.per_page ?? perPage,
          total: query.metadata?.total ?? 0,
        },
      },
    };
  };

  return (
    <div className="grid gap-6 p-6">
      <GenericList
        title={
          <span className="text-xs font-medium uppercase tracking-[0.24em] text-base-content/50">
            Activities
          </span>
        }
        paramsSchema={AdminActivitiesGridSchema}
        useQuery={useAdminActivitiesGridQuery}
        columns={columns}
        emptyMessage="No activities found."
      />

      <ActivityImportTraceModal
        activity={selectedActivity}
        trace={traceQuery.data}
        isLoading={traceQuery.isLoading}
        error={traceQuery.error}
        onClose={() => {
          setSelectedActivity(null);
        }}
      />
    </div>
  );
}

function ActivityImportTraceModal({
  activity,
  trace,
  isLoading,
  error,
  onClose,
}: {
  activity: AdminActivity | null;
  trace: ReturnType<typeof useActivityImportTrace>["data"];
  isLoading: boolean;
  error: Error | null;
  onClose: () => void;
}) {
  const isOpen = !!activity;

  if (!isOpen) {
    return null;
  }

  return (
    <dialog className="modal modal-open">
      <div className="modal-box max-w-5xl">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="text-lg font-semibold">Activity processing</h2>
            {activity?.activity_import_id ? (
              <p className="mt-1 text-sm text-base-content/65">
                Activity #{activity.id} · import #{activity.activity_import_id}
                {activity.import_version
                  ? ` · version ${activity.import_version}`
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
          {activity ? <ActivityPipelines activityId={activity.id} /> : null}
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

function positiveNumber(value: number | string | undefined, fallback: number) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function importStatusClass(status: string | null | undefined) {
  if (status === "processed") {
    return "badge badge-success badge-outline";
  }
  if (status === "failed") {
    return "badge badge-error badge-outline";
  }
  if (status === "duplicate") {
    return "badge badge-warning badge-outline";
  }
  return "badge badge-info badge-outline";
}
