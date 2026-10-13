"use client";

import Link from "next/link";
import { useActivityImportTrace } from "../../lib/queries";
import ActivityImportTracePanel from "../activity-detail/ActivityImportTracePanel";
import { AppCard } from "../ui/Card";
import ActivityPipelines from "../admin/ActivityPipelines";
import { ProcessingFacts, ProcessingStatus } from "../ui/ProcessingDetails";
import { formatTaskDate } from "../../lib/taskTiming";

export default function ImportDetail({ importId }: { importId: string }) {
  const query = useActivityImportTrace(importId);
  return (
    <AppCard bodyClassName="gap-6">
      <Link href="/admin/imports" className="link">
        ← Import history
      </Link>
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <p className="mb-2 text-sm font-medium text-base-content/60">
            Activity processing · Import #{importId}
          </p>
          <h1 className="text-2xl font-semibold">
            {query.data?.import.original_filename ?? `Import #${importId}`}
          </h1>
          {query.data ? (
            <div className="mt-3">
              <ProcessingStatus status={query.data.import.status} />
            </div>
          ) : null}
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            className="btn btn-outline"
            disabled={query.isFetching}
            onClick={() => void query.refetch()}
          >
            Refresh
          </button>
          {query.data?.import.activity_id ? (
            <Link
              className="btn btn-outline"
              href={`/activities/${query.data.import.activity_id}`}
            >
              View activity
            </Link>
          ) : null}
        </div>
      </div>
      {query.data ? (
        <section
          aria-label="Import overview"
          className="rounded-box border border-base-300 p-4"
        >
          <ProcessingFacts
            facts={[
              {
                label: "Source",
                value: query.data.import.source?.replaceAll("_", " "),
              },
              {
                label: "Size",
                value:
                  query.data.import.size_bytes == null
                    ? null
                    : `${query.data.import.size_bytes.toLocaleString()} bytes`,
              },
              {
                label: "Received",
                value: formatTaskDate(query.data.import.created_at) || null,
              },
              {
                label: "Activity",
                value: query.data.import.activity_id
                  ? `#${query.data.import.activity_id}`
                  : "No activity created",
              },
            ]}
          />
        </section>
      ) : null}
      <nav aria-label="Processing sections" className="flex flex-wrap gap-2">
        <a href="#import-stages" className="btn btn-sm btn-outline">
          Import stages and replay
        </a>
        <a href="#worker-runs" className="btn btn-sm btn-outline">
          Worker runs and publication
        </a>
      </nav>
      <section id="import-stages" className="scroll-mt-4">
        <ActivityImportTracePanel
          trace={query.data}
          isLoading={query.isLoading}
          error={query.error}
          canReplay
        />
      </section>
      <section
        id="worker-runs"
        className="scroll-mt-4 border-t border-base-300 pt-6"
      >
        <h2 className="text-lg font-semibold">Worker runs and publication</h2>
        <p className="mt-1 mb-4 text-sm text-base-content/65">
          Follow the background work and output readiness for this activity.
        </p>
        {query.data?.import.activity_id ? (
          <ActivityPipelines
            key={query.data.import.activity_id}
            activityId={query.data.import.activity_id}
          />
        ) : (
          <p className="text-sm text-base-content/65">
            Worker runs will appear here when this import is linked to an
            activity.
          </p>
        )}
      </section>
    </AppCard>
  );
}
