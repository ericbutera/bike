"use client";

import IntegrationEventFeed from "@/components/IntegrationEventFeed";
import { useAdminIntegrationEvents } from "@/lib/queries";
import { admin } from "@ericbutera/kaleido";
import { useRouter, useSearchParams } from "next/navigation";
import { Suspense, useDeferredValue, useEffect, useState } from "react";
import AuthRouter from "../../../components/AuthRouter";

export default function AdminIntegrationsPage() {
  return (
    <Suspense>
      <AuthRouter>
        <admin.Layout title="Integrations">
          <AdminIntegrationsContent />
        </admin.Layout>
      </AuthRouter>
    </Suspense>
  );
}

function AdminIntegrationsContent() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const [userIdFilter, setUserIdFilter] = useState(
    searchParams.get("user_id") ?? "",
  );
  const [activityIdFilter, setActivityIdFilter] = useState(
    searchParams.get("activity_id") ?? "",
  );
  const [importIdFilter, setImportIdFilter] = useState(
    searchParams.get("import_id") ?? "",
  );
  const [providerFilter, setProviderFilter] = useState(
    searchParams.get("provider") ?? "strava",
  );
  const deferredUserIdFilter = useDeferredValue(userIdFilter);
  const deferredActivityIdFilter = useDeferredValue(activityIdFilter);
  const deferredImportIdFilter = useDeferredValue(importIdFilter);
  const trimmedUserIdFilter = deferredUserIdFilter.trim();
  const trimmedActivityIdFilter = deferredActivityIdFilter.trim();
  const trimmedImportIdFilter = deferredImportIdFilter.trim();
  const parsedUserId = Number.parseInt(trimmedUserIdFilter, 10);
  const parsedActivityId = Number.parseInt(trimmedActivityIdFilter, 10);
  const parsedImportId = Number.parseInt(trimmedImportIdFilter, 10);
  const hasInvalidUserId =
    trimmedUserIdFilter.length > 0 &&
    (!Number.isFinite(parsedUserId) || parsedUserId < 1);
  const hasInvalidActivityId =
    trimmedActivityIdFilter.length > 0 &&
    (!Number.isFinite(parsedActivityId) || parsedActivityId < 1);
  const hasInvalidImportId =
    trimmedImportIdFilter.length > 0 &&
    (!Number.isFinite(parsedImportId) || parsedImportId < 1);

  useEffect(() => {
    const params = new URLSearchParams();
    if (providerFilter) {
      params.set("provider", providerFilter);
    }
    if (!hasInvalidUserId && trimmedUserIdFilter) {
      params.set("user_id", trimmedUserIdFilter);
    }
    if (!hasInvalidActivityId && trimmedActivityIdFilter) {
      params.set("activity_id", trimmedActivityIdFilter);
    }
    if (!hasInvalidImportId && trimmedImportIdFilter) {
      params.set("import_id", trimmedImportIdFilter);
    }

    const nextQuery = params.toString();
    const currentQuery = searchParams.toString();
    if (nextQuery !== currentQuery) {
      router.replace(`/admin/integrations${nextQuery ? `?${nextQuery}` : ""}`, {
        scroll: false,
      });
    }
  }, [
    hasInvalidActivityId,
    hasInvalidImportId,
    hasInvalidUserId,
    providerFilter,
    router,
    searchParams,
    trimmedActivityIdFilter,
    trimmedImportIdFilter,
    trimmedUserIdFilter,
  ]);

  const eventsQuery = useAdminIntegrationEvents({
    provider: providerFilter || undefined,
    userId: hasInvalidUserId || !trimmedUserIdFilter ? null : parsedUserId,
    activityId:
      hasInvalidActivityId || !trimmedActivityIdFilter
        ? null
        : parsedActivityId,
    importId: hasInvalidImportId || !trimmedImportIdFilter ? null : parsedImportId,
    limit: 100,
    refetchIntervalMs: 5000,
  });

  return (
    <div className="grid gap-6 p-6">
      <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h2 className="text-lg font-semibold">Integration events</h2>
            <p className="mt-2 max-w-3xl text-sm text-base-content/70">
              Review webhook deliveries, import checkpoints, sync lifecycle
              changes, duplicate matches, and processing failures from the Bike
              admin area.
            </p>
          </div>
          <span className="badge badge-outline uppercase">
            {providerFilter || "All providers"}
          </span>
        </div>

        <div className="mt-6 grid gap-4 md:grid-cols-2 xl:grid-cols-[minmax(0,16rem)_minmax(0,14rem)_minmax(0,14rem)_minmax(0,14rem)_auto] xl:items-end">
          <label className="form-control">
            <div className="label">
              <span className="label-text font-medium">Provider</span>
            </div>
            <select
              className="select select-bordered"
              value={providerFilter}
              onChange={(event) => {
                setProviderFilter(event.target.value);
              }}
            >
              <option value="strava">Strava</option>
              <option value="activity_processing">Activity processing</option>
              <option value="">All providers</option>
            </select>
            <div className="label min-h-6" />
          </label>

          <label className="form-control">
            <div className="label">
              <span className="label-text font-medium">Filter by user id</span>
            </div>
            <input
              type="number"
              min={1}
              className="input input-bordered"
              placeholder="Any user"
              value={userIdFilter}
              onChange={(event) => {
                setUserIdFilter(event.target.value);
              }}
            />
            <div className="label min-h-6">
              <span className="label-text-alt text-error">
                {hasInvalidUserId ? "Enter a positive integer user id." : ""}
              </span>
            </div>
          </label>

          <label className="form-control">
            <div className="label">
              <span className="label-text font-medium">Activity id</span>
            </div>
            <input
              type="number"
              min={1}
              className="input input-bordered"
              placeholder="Any activity"
              value={activityIdFilter}
              onChange={(event) => {
                setActivityIdFilter(event.target.value);
              }}
            />
            <div className="label min-h-6">
              <span className="label-text-alt text-error">
                {hasInvalidActivityId ? "Enter a positive integer activity id." : ""}
              </span>
            </div>
          </label>

          <label className="form-control">
            <div className="label">
              <span className="label-text font-medium">Import id</span>
            </div>
            <input
              type="number"
              min={1}
              className="input input-bordered"
              placeholder="Any import"
              value={importIdFilter}
              onChange={(event) => {
                setImportIdFilter(event.target.value);
              }}
            />
            <div className="label min-h-6">
              <span className="label-text-alt text-error">
                {hasInvalidImportId ? "Enter a positive integer import id." : ""}
              </span>
            </div>
          </label>

          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              className="btn btn-ghost"
              disabled={!userIdFilter && !activityIdFilter && !importIdFilter}
              onClick={() => {
                setUserIdFilter("");
                setActivityIdFilter("");
                setImportIdFilter("");
              }}
            >
              Clear ids
            </button>
          </div>
        </div>
      </section>

      <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
        <IntegrationEventFeed
          events={eventsQuery.data}
          isLoading={eventsQuery.isLoading}
          error={eventsQuery.error}
          emptyMessage="No integration events matched the current filter."
          showUserId
          showProvider
        />
      </section>
    </div>
  );
}
