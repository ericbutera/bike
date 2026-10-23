"use client";

import { useSearchParams } from "next/navigation";
import type { ReactNode } from "react";
import { useActivity } from "../lib/queries";
import SegmentBuilderWorkspace from "./SegmentBuilderWorkspace";
import { AppCard } from "./ui/Card";
import { ErrorCard } from "./ui/QueryState";

function parseNumericId(rawValue: string | null) {
  const parsed = Number(rawValue);

  if (!Number.isInteger(parsed) || parsed <= 0) {
    return null;
  }

  return parsed;
}

function SegmentBuilderPageShell({ children }: { children: ReactNode }) {
  return (
    <section className="space-y-6">
      <AppCard bodyClassName="gap-4">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h1 className="text-4xl font-semibold tracking-tight text-base-content">
              Segment builder
            </h1>
          </div>
        </div>
      </AppCard>

      {children}
    </section>
  );
}

function NewSegmentBuilderPage({ activityId }: { activityId: number | null }) {
  const activityQuery = useActivity(activityId);

  return (
    <SegmentBuilderPageShell>
      <SegmentBuilderWorkspace
        activity={activityQuery.data}
        isLoading={activityQuery.isLoading}
      />
    </SegmentBuilderPageShell>
  );
}

export default function SegmentBuilderPage() {
  const searchParams = useSearchParams();
  const selectedSegmentId = parseNumericId(searchParams.get("segmentId"));
  const requestedActivityId = parseNumericId(searchParams.get("activityId"));

  return selectedSegmentId != null ? (
    <SegmentBuilderPageShell>
      <ErrorCard fallback="Saved segment geometry is locked. Rename the segment from its detail page." />
    </SegmentBuilderPageShell>
  ) : (
    <NewSegmentBuilderPage activityId={requestedActivityId} />
  );
}
