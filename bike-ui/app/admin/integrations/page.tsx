"use client";

import IntegrationEventFeed from "@/components/IntegrationEventFeed";
import { ListCard } from "@/components/ui/DataGrid";
import { useAdminIntegrationEvents } from "@/lib/queries";
import { useRouter, useSearchParams } from "next/navigation";
import { Suspense, useDeferredValue, useEffect, useState } from "react";
import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";

export default function AdminIntegrationsPage() {
  return (
    <Suspense>
      <LocalAdminLayout title="Integrations">
        <AdminIntegrationsContent />
      </LocalAdminLayout>
    </Suspense>
  );
}

const emptyIds = { user_id: "", activity_id: "", import_id: "" };

function validId(value: string) {
  return (
    !value.trim() || (Number.isInteger(Number(value)) && Number(value) > 0)
  );
}

function AdminIntegrationsContent() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const [ids, setIds] = useState({
    user_id: searchParams.get("user_id") ?? "",
    activity_id: searchParams.get("activity_id") ?? "",
    import_id: searchParams.get("import_id") ?? "",
  });
  const [provider, setProvider] = useState(
    searchParams.get("provider") ?? "strava",
  );
  const [search, setSearch] = useState("");
  const deferredIds = useDeferredValue(ids);
  const eventsQuery = useAdminIntegrationEvents({
    provider: provider || undefined,
    userId:
      validId(deferredIds.user_id) && deferredIds.user_id
        ? Number(deferredIds.user_id)
        : null,
    activityId:
      validId(deferredIds.activity_id) && deferredIds.activity_id
        ? Number(deferredIds.activity_id)
        : null,
    importId:
      validId(deferredIds.import_id) && deferredIds.import_id
        ? Number(deferredIds.import_id)
        : null,
    enabled: Object.values(deferredIds).every(validId),
    limit: 100,
    refetchIntervalMs: 5000,
  });

  useEffect(() => {
    const params = new URLSearchParams();
    params.set("provider", provider);
    Object.entries(deferredIds).forEach(([key, value]) => {
      if (value.trim() && validId(value)) params.set(key, value.trim());
    });
    const nextQuery = params.toString();
    if (nextQuery !== searchParams.toString()) {
      router.replace(`/admin/integrations${nextQuery ? `?${nextQuery}` : ""}`, {
        scroll: false,
      });
    }
  }, [deferredIds, provider, router, searchParams]);

  const term = search.trim().toLowerCase();
  const events = (eventsQuery.data ?? []).filter(
    (event) =>
      !term ||
      [
        event.message,
        event.event_type,
        event.level,
        JSON.stringify(event.payload),
      ]
        .join(" ")
        .toLowerCase()
        .includes(term),
  );

  return (
    <div className="grid gap-6">
      <ListCard
        title="Integration events"
        onSearch={() => {
          if (Object.values(ids).every(validId)) void eventsQuery.refetch();
        }}
        onClear={() => {
          setIds(emptyIds);
          setProvider("");
          setSearch("");
        }}
        filters={
          <>
            <select
              aria-label="Provider"
              className="select select-sm select-bordered w-44"
              value={provider}
              onChange={(event) => setProvider(event.target.value)}
            >
              <option value="">All providers</option>
              <option value="strava">Strava</option>
              <option value="activity_processing">Activity processing</option>
            </select>
            {(
              [
                ["user_id", "User ID", "Any user"],
                ["activity_id", "Activity ID", "Any activity"],
                ["import_id", "Import ID", "Any import"],
              ] as const
            ).map(([key, label, placeholder]) => (
              <input
                key={key}
                type="number"
                min={1}
                step={1}
                aria-label={label}
                aria-invalid={!validId(ids[key])}
                title={label}
                placeholder={placeholder}
                className="input input-sm input-bordered w-32"
                value={ids[key]}
                onChange={(event) =>
                  setIds((current) => ({
                    ...current,
                    [key]: event.target.value,
                  }))
                }
              />
            ))}
            <input
              type="search"
              aria-label="Search recent integration events"
              placeholder="Search recent events"
              title="Search the latest 100 matching events"
              className="input input-sm input-bordered w-56"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
          </>
        }
      >
        {!Object.values(ids).every(validId) ? (
          <p role="alert" className="text-error">
            IDs must be positive integers.
          </p>
        ) : (
          <IntegrationEventFeed
            audience="admin"
            events={events}
            isLoading={eventsQuery.isLoading}
            error={eventsQuery.error}
            emptyMessage="No integration events matched the current filter."
            showUserId
            showProvider
          />
        )}
      </ListCard>
    </div>
  );
}
