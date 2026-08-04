"use client";

import { useSearchParams } from "next/navigation";
import type { ReactNode } from "react";
import { useActivity, useSegment } from "../lib/queries";
import SegmentBuilderWorkspace from "./SegmentBuilderWorkspace";

function parseNumericId(rawValue: string | null) {
  const parsed = Number(rawValue);

  if (!Number.isInteger(parsed) || parsed <= 0) {
    return null;
  }

  return parsed;
}

function SegmentBuilderPageShell({
  isEditingExistingSegment,
  children,
}: {
  isEditingExistingSegment: boolean;
  children: ReactNode;
}) {
  return (
    <section className="space-y-6">
      <div className="card bg-base-100 shadow-xl">
        <div className="card-body gap-4">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <h1 className="text-4xl font-semibold tracking-tight text-base-content">
                Segment builder
              </h1>
            </div>
          </div>
        </div>
      </div>

      {children}
    </section>
  );
}

function NewSegmentBuilderPage({ activityId }: { activityId: number | null }) {
  const activityQuery = useActivity(activityId);

  return (
    <SegmentBuilderPageShell isEditingExistingSegment={false}>
      <SegmentBuilderWorkspace
        segment={null}
        activity={activityQuery.data}
        isLoading={activityQuery.isLoading}
      />
    </SegmentBuilderPageShell>
  );
}

function EditSegmentBuilderPage({ segmentId }: { segmentId: number }) {
  const segmentQuery = useSegment(segmentId);
  const selectedActivityId =
    segmentQuery.data?.builder_source?.activity_id ?? null;
  const activityQuery = useActivity(selectedActivityId);
  const builderStateError =
    segmentQuery.data && !segmentQuery.data.builder_source
      ? new Error(
          "This segment cannot be reopened in the builder because it was not created from a ride crop.",
        )
      : null;

  return (
    <SegmentBuilderPageShell isEditingExistingSegment>
      <SegmentBuilderWorkspace
        segment={segmentQuery.data ?? null}
        activity={activityQuery.data}
        isLoading={segmentQuery.isLoading || activityQuery.isLoading}
        unavailableMessage={builderStateError?.message}
      />
    </SegmentBuilderPageShell>
  );
}

export default function SegmentBuilderPage() {
  const searchParams = useSearchParams();
  const selectedSegmentId = parseNumericId(searchParams.get("segmentId"));
  const requestedActivityId = parseNumericId(searchParams.get("activityId"));

  return selectedSegmentId != null ? (
    <EditSegmentBuilderPage segmentId={selectedSegmentId} />
  ) : (
    <NewSegmentBuilderPage activityId={requestedActivityId} />
  );
}
