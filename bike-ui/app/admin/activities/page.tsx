"use client";

import { type AdminActivity, useAdminActivities } from "@/lib/queries";
import GenericList, { type Column } from "@/components/ui/GenericList";
import { faGear } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { Suspense, useMemo } from "react";
import Link from "next/link";
import { ACTIVITY_SPORT_OPTIONS } from "@/lib/activitySports";
import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";

const ADMIN_ACTIVITIES_PAGE_SIZE = 25;
const AdminActivitiesGridSchema = {} as never;

type AdminActivitiesGridParams = {
  page?: number | string;
  per_page?: number | string;
  q?: string;
  user_id?: string;
  source?: string;
  sport?: string;
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
  const columns = useMemo<Column<AdminActivity, AdminActivitiesGridParams>[]>(
    () => [
      { key: "id", header: "ID", render: (activity) => activity.id },
      {
        key: "title",
        header: "Activity",
        className: "max-w-xs truncate",
        render: (activity) => (
          <Link
            title={activity.title}
            className="link link-primary"
            href={`/activities/${activity.id}`}
          >
            {activity.title}
          </Link>
        ),
      },
      {
        key: "user_id",
        header: "User",
        render: (activity) => activity.user_id,
      },
      {
        key: "sport",
        header: "Sport",
        className: "whitespace-nowrap",
        render: (activity) => activity.sport,
      },
      {
        key: "started_at",
        header: "Started",
        className: "whitespace-nowrap",
        render: (activity) => formatDateTime(activity.started_at),
      },
      {
        key: "source",
        header: "Source",
        className: "whitespace-nowrap",
        render: (activity) => (
          <span className="badge badge-outline">{activity.source}</span>
        ),
      },
      {
        key: "format",
        header: "Format",
        render: (activity) => activity.format?.toUpperCase() || "—",
      },
      {
        key: "activity_import_id",
        header: "Import",
        render: (activity) =>
          activity.activity_import_id ? (
            <Link
              className="link"
              href={`/admin/imports/${activity.activity_import_id}`}
            >
              #{activity.activity_import_id}
            </Link>
          ) : (
            "None"
          ),
      },
      {
        key: "import_status",
        header: "Import status",
        className: "whitespace-nowrap",
        render: (activity) =>
          activity.import_status ? (
            <span
              title={activity.import_processing_stage ?? undefined}
              className={importStatusClass(activity.import_status)}
            >
              {activity.import_status}
            </span>
          ) : (
            "—"
          ),
      },
      {
        key: "actions",
        header: "",
        className: "w-12 whitespace-nowrap",
        render: (activity) =>
          activity.activity_import_id ? (
            <Link
              className="btn btn-ghost btn-square btn-sm"
              href={`/admin/imports/${activity.activity_import_id}`}
              aria-label={`View import details for ${activity.title}`}
              title="Import details"
            >
              <FontAwesomeIcon icon={faGear} className="h-4 w-4" />
            </Link>
          ) : null,
      },
    ],
    [],
  );

  const useAdminActivitiesGridQuery = (params: AdminActivitiesGridParams) => {
    const page = positiveNumber(params.page, 1);
    const perPage = positiveNumber(params.per_page, ADMIN_ACTIVITIES_PAGE_SIZE);
    const validUserId =
      !params.user_id ||
      (Number.isInteger(Number(params.user_id)) && Number(params.user_id) > 0);
    const query = useAdminActivities({
      page,
      perPage,
      q: params.q,
      userId: params.user_id ? Number(params.user_id) : undefined,
      source: params.source,
      sport: params.sport,
      enabled: validUserId,
    });

    return {
      data: query.data ?? [],
      isLoading: query.isLoading,
      error: validUserId ? query.error : new Error("User ID must be positive"),
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
    <div className="grid gap-6">
      <GenericList
        title="Activities"
        paramsSchema={AdminActivitiesGridSchema}
        useQuery={useAdminActivitiesGridQuery}
        columns={columns}
        renderFilters={(params, setFilter) => (
          <>
            <input
              type="search"
              aria-label="Search activities"
              placeholder="Search title or activity ID"
              className="input input-sm input-bordered w-56"
              value={params.q ?? ""}
              onChange={(event) => setFilter("q", event.target.value)}
            />
            <input
              type="number"
              min={1}
              step={1}
              aria-label="Activity owner ID"
              title="Activity owner ID"
              placeholder="Any user"
              className="input input-sm input-bordered w-32"
              value={params.user_id ?? ""}
              onChange={(event) => setFilter("user_id", event.target.value)}
            />
            <select
              aria-label="Activity source"
              className="select select-sm select-bordered w-40"
              value={params.source ?? ""}
              onChange={(event) => setFilter("source", event.target.value)}
            >
              <option value="">All sources</option>
              <option value="manual_upload">Upload</option>
              <option value="archive_url_import">Archive</option>
              <option value="strava_sync">Strava</option>
            </select>
            <select
              aria-label="Activity sport"
              className="select select-sm select-bordered w-40"
              value={params.sport ?? ""}
              onChange={(event) => setFilter("sport", event.target.value)}
            >
              <option value="">All sports</option>
              {ACTIVITY_SPORT_OPTIONS.map((sport) => (
                <option key={sport.value} value={sport.value}>
                  {sport.label}
                </option>
              ))}
            </select>
          </>
        )}
        emptyMessage="No activities found matching criteria."
      />
    </div>
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
