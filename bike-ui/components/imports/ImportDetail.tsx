"use client";

import Link from "next/link";
import { useAdminActivityImport } from "../../lib/queries";
import { AppCard } from "../ui/Card";
import { ProcessingFacts, ProcessingStatus } from "../ui/ProcessingDetails";
import { ErrorCard, LoadingCard } from "../ui/QueryState";
import TemporalLink from "../ui/TemporalLink";
import { formatTaskDate } from "../../lib/taskTiming";
import RefreshButton from "../ui/RefreshButton";

export default function ImportDetail({ importId }: { importId: string }) {
  const query = useAdminActivityImport(importId);
  const imported = query.data;
  return (
    <AppCard bodyClassName="gap-6">
      <Link href="/admin/imports" className="link">
        ← Import history
      </Link>
      <div className="flex flex-wrap items-center justify-between gap-4">
        <h1 className="text-2xl font-semibold">
          {imported?.original_filename ?? `Import #${importId}`}
        </h1>
        <RefreshButton
          disabled={query.isFetching}
          onClick={() => void query.refetch()}
        />
      </div>
      {query.isLoading ? <LoadingCard /> : null}
      {query.error ? (
        <ErrorCard
          error={query.error}
          fallback="Unable to load import details."
        />
      ) : null}
      {imported ? (
        <section aria-label="Import overview" className="space-y-4">
          <ProcessingStatus status={imported.status} />
          <ProcessingFacts
            facts={[
              { label: "Source", value: imported.source?.replaceAll("_", " ") },
              {
                label: "Size",
                value: `${imported.size_bytes.toLocaleString()} bytes`,
              },
              {
                label: "Received",
                value: formatTaskDate(imported.created_at) || null,
              },
              {
                label: "Activity",
                value: imported.activity_id
                  ? `#${imported.activity_id}`
                  : "No activity created",
              },
            ]}
          />
          {imported.processing_error ? (
            <p role="alert">{imported.processing_error}</p>
          ) : null}
          {imported.activity_id ? (
            <Link
              className="btn btn-outline"
              href={`/activities/${imported.activity_id}`}
            >
              View activity
            </Link>
          ) : null}
        </section>
      ) : null}
      <p>Inspect workflow execution, retries, and cancellation in Temporal.</p>
      <TemporalLink />
    </AppCard>
  );
}
