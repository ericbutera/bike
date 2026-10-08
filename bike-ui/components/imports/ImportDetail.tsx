"use client";

import Link from "next/link";
import { useActivityImportTrace } from "../../lib/queries";
import ActivityImportTracePanel from "../activity-detail/ActivityImportTracePanel";
import { AppCard } from "../ui/Card";

export default function ImportDetail({ importId }: { importId: string }) {
  const query = useActivityImportTrace(importId);
  return (
    <AppCard>
      <Link href="/imports" className="link">
        ← Import history
      </Link>
      <div className="my-6 flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">
            {query.data?.import.original_filename ?? `Import #${importId}`}
          </h1>
          {query.data ? (
            <p className="mt-2 text-base-content/70">
              {query.data.import.status} · {query.data.import.source} ·{" "}
              {query.data.import.size_bytes?.toLocaleString() ?? "Unknown"}{" "}
              bytes
            </p>
          ) : null}
        </div>
        {query.data?.import.activity_id ? (
          <Link
            className="btn btn-outline"
            href={`/activities/${query.data.import.activity_id}`}
          >
            View activity
          </Link>
        ) : null}
      </div>
      <ActivityImportTracePanel
        trace={query.data}
        isLoading={query.isLoading}
        error={query.error}
        canReplay
      />
    </AppCard>
  );
}
