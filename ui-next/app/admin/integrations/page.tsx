"use client";

import IntegrationEventFeed from "@/components/IntegrationEventFeed";
import { useAdminIntegrationEvents } from "@/lib/queries";
import { admin } from "@ericbutera/kaleido";
import { Suspense, useDeferredValue, useState } from "react";
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
  const [userIdFilter, setUserIdFilter] = useState("");
  const deferredUserIdFilter = useDeferredValue(userIdFilter);
  const trimmedUserIdFilter = deferredUserIdFilter.trim();
  const parsedUserId = Number.parseInt(trimmedUserIdFilter, 10);
  const hasInvalidUserId =
    trimmedUserIdFilter.length > 0 &&
    (!Number.isFinite(parsedUserId) || parsedUserId < 1);

  const eventsQuery = useAdminIntegrationEvents({
    provider: "strava",
    userId: hasInvalidUserId || !trimmedUserIdFilter ? null : parsedUserId,
    limit: 100,
    refetchIntervalMs: 5000,
  });

  return (
    <div className="grid gap-6 p-6">
      <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h2 className="text-lg font-semibold">Strava integration events</h2>
            <p className="mt-2 max-w-3xl text-sm text-base-content/70">
              Review webhook deliveries, OAuth transitions, sync lifecycle
              changes, and disconnect events from the Bike admin area.
            </p>
          </div>
          <span className="badge badge-outline uppercase">Strava</span>
        </div>

        <div className="mt-6 grid gap-4 md:grid-cols-[minmax(0,18rem)_auto] md:items-end">
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

          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              className="btn btn-ghost"
              disabled={!userIdFilter}
              onClick={() => {
                setUserIdFilter("");
              }}
            >
              Clear filter
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
