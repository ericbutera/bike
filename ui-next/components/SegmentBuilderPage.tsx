"use client";

import { auth } from "@ericbutera/kaleido";
import { useSearchParams } from "next/navigation";
import type { ReactNode } from "react";
import { useActivity, useSegment } from "../lib/queries";
import AuthRequiredCard from "./AuthRequiredCard";
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
        isError={activityQuery.isError}
        error={activityQuery.error}
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
        isError={
          Boolean(builderStateError) ||
          segmentQuery.isError ||
          activityQuery.isError
        }
        error={builderStateError ?? segmentQuery.error ?? activityQuery.error}
      />
    </SegmentBuilderPageShell>
  );
}

export default function SegmentBuilderPage() {
  const authApi = auth.useAuthApi();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const searchParams = useSearchParams();
  const selectedSegmentId = parseNumericId(searchParams.get("segmentId"));
  const requestedActivityId = parseNumericId(searchParams.get("activityId"));

  if (isLoadingUser) {
    return (
      <section className="card bg-base-100 shadow-xl">
        <div className="card-body items-center py-16">
          <span className="loading loading-spinner loading-lg" />
        </div>
      </section>
    );
  }

  if (!user) {
    return (
      <AuthRequiredCard
        eyebrow="Segment builder"
        title="Build segments from your rides"
        description="Sign in to crop one of your uploaded rides into a segment, save it, and jump directly into the comparison workspace."
        showSignup
      />
    );
  }

  return selectedSegmentId != null ? (
    <EditSegmentBuilderPage segmentId={selectedSegmentId} />
  ) : (
    <NewSegmentBuilderPage activityId={requestedActivityId} />
  );
}
