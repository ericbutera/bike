"use client";

import { extractApiMessage } from "@/lib/activityFormatting";
import {
  useActivityArchiveImportJobs,
  useAdminBackfillAnalytics,
  useAdminBackfillUserXcTraining,
  useCleanupUserDuplicateActivities,
  useImportActivityArchiveUrl,
  useRegenerateSegmentEfforts,
  useRegenerateUserSegments,
  useReprocessUserActivityImports,
  type ActivityArchiveImportJob,
  type AdminAnalyticsBackfillResponse,
  type CleanupUserDuplicateActivitiesResponse,
  type RegenerateSegmentEffortsResponse,
  type RegenerateUserSegmentsResponse,
  type ReprocessUserActivityImportsResponse,
} from "@/lib/queries";
import Link from "next/link";
import { useState } from "react";

export default function AdminTaskTools() {
  const { backfillAsync, isPending } = useAdminBackfillAnalytics();
  const archiveJobsQuery = useActivityArchiveImportJobs({
    enabled: true,
    refetchIntervalMs: 5000,
  });
  const { importAsync, isPending: isImportPending } =
    useImportActivityArchiveUrl();
  const { cleanupAsync, isPending: isCleanupPending } =
    useCleanupUserDuplicateActivities();
  const { reprocessAsync, isPending: isReprocessPending } =
    useReprocessUserActivityImports();
  const {
    regenerateAsync: regenerateUserSegmentsAsync,
    isPending: isUserSegmentRegenerationPending,
  } = useRegenerateUserSegments();
  const {
    regenerateAsync: regenerateSegmentEffortsAsync,
    isPending: isSegmentEffortRegenerationPending,
  } = useRegenerateSegmentEfforts();
  const {
    backfillAsync: backfillXcTrainingAsync,
    isPending: isXcBackfillPending,
  } = useAdminBackfillUserXcTraining();
  const [result, setResult] = useState<AdminAnalyticsBackfillResponse | null>(
    null,
  );
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [archiveUrl, setArchiveUrl] = useState("");
  const [archiveResult, setArchiveResult] =
    useState<ActivityArchiveImportJob | null>(null);
  const [archiveErrorMessage, setArchiveErrorMessage] = useState<string | null>(
    null,
  );
  const [reprocessUserId, setReprocessUserId] = useState("");
  const [reprocessResult, setReprocessResult] =
    useState<ReprocessUserActivityImportsResponse | null>(null);
  const [reprocessErrorMessage, setReprocessErrorMessage] = useState<
    string | null
  >(null);
  const [userSegmentUserId, setUserSegmentUserId] = useState("");
  const [userSegmentResult, setUserSegmentResult] =
    useState<RegenerateUserSegmentsResponse | null>(null);
  const [userSegmentErrorMessage, setUserSegmentErrorMessage] = useState<
    string | null
  >(null);
  const [segmentEffortSegmentId, setSegmentEffortSegmentId] = useState("");
  const [segmentEffortResult, setSegmentEffortResult] =
    useState<RegenerateSegmentEffortsResponse | null>(null);
  const [segmentEffortErrorMessage, setSegmentEffortErrorMessage] = useState<
    string | null
  >(null);
  const [xcBackfillUserId, setXcBackfillUserId] = useState("");
  const [xcBackfillResult, setXcBackfillResult] =
    useState<ReprocessUserActivityImportsResponse | null>(null);
  const [xcBackfillErrorMessage, setXcBackfillErrorMessage] = useState<
    string | null
  >(null);
  const [cleanupUserId, setCleanupUserId] = useState("");
  const [cleanupResult, setCleanupResult] =
    useState<CleanupUserDuplicateActivitiesResponse | null>(null);
  const [cleanupErrorMessage, setCleanupErrorMessage] = useState<string | null>(
    null,
  );

  async function handleBackfill() {
    setErrorMessage(null);

    try {
      const response = await backfillAsync();
      setResult(response);
    } catch (error) {
      setErrorMessage(
        error instanceof Error
          ? error.message
          : "Failed to enqueue analytics backfill",
      );
    }
  }

  async function handleArchiveImport() {
    setArchiveErrorMessage(null);

    try {
      const response = await importAsync(archiveUrl.trim());
      setArchiveResult(response);
    } catch (error) {
      setArchiveErrorMessage(extractApiMessage(error));
    }
  }

  async function handleReprocessImports() {
    const numericUserId = Number(reprocessUserId);

    setReprocessErrorMessage(null);

    if (!Number.isFinite(numericUserId) || numericUserId < 1) {
      setReprocessResult(null);
      setReprocessErrorMessage("Enter a valid user id.");
      return;
    }

    try {
      const response = await reprocessAsync(numericUserId);
      setReprocessResult(response);
    } catch (error) {
      setReprocessResult(null);
      setReprocessErrorMessage(extractApiMessage(error));
    }
  }

  async function handleUserSegmentRegeneration() {
    const numericUserId = Number(userSegmentUserId);

    setUserSegmentErrorMessage(null);

    if (!Number.isFinite(numericUserId) || numericUserId < 1) {
      setUserSegmentResult(null);
      setUserSegmentErrorMessage("Enter a valid user id.");
      return;
    }

    try {
      const response = await regenerateUserSegmentsAsync(numericUserId);
      setUserSegmentResult(response);
    } catch (error) {
      setUserSegmentResult(null);
      setUserSegmentErrorMessage(extractApiMessage(error));
    }
  }

  async function handleSegmentEffortRegeneration() {
    const numericSegmentId = Number(segmentEffortSegmentId);

    setSegmentEffortErrorMessage(null);

    if (!Number.isFinite(numericSegmentId) || numericSegmentId < 1) {
      setSegmentEffortResult(null);
      setSegmentEffortErrorMessage("Enter a valid segment id.");
      return;
    }

    try {
      const response = await regenerateSegmentEffortsAsync(numericSegmentId);
      setSegmentEffortResult(response);
    } catch (error) {
      setSegmentEffortResult(null);
      setSegmentEffortErrorMessage(extractApiMessage(error));
    }
  }

  async function handleCleanupDuplicates() {
    const numericUserId = Number(cleanupUserId);

    setCleanupErrorMessage(null);

    if (!Number.isFinite(numericUserId) || numericUserId < 1) {
      setCleanupResult(null);
      setCleanupErrorMessage("Enter a valid user id.");
      return;
    }

    try {
      const response = await cleanupAsync(numericUserId);
      setCleanupResult(response);
    } catch (error) {
      setCleanupResult(null);
      setCleanupErrorMessage(extractApiMessage(error));
    }
  }

  async function handleXcTrainingBackfill() {
    const numericUserId = Number(xcBackfillUserId);

    setXcBackfillErrorMessage(null);

    if (!Number.isFinite(numericUserId) || numericUserId < 1) {
      setXcBackfillResult(null);
      setXcBackfillErrorMessage("Enter a valid user id.");
      return;
    }

    try {
      const response = await backfillXcTrainingAsync(numericUserId);
      setXcBackfillResult(response);
    } catch (error) {
      setXcBackfillResult(null);
      setXcBackfillErrorMessage(extractApiMessage(error));
    }
  }

  return (
    <div className="grid gap-6">
      <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
        <h2 className="text-lg font-semibold">Regenerate segment efforts</h2>
        <p className="mt-2 max-w-3xl text-sm text-base-content/70">
          Rebuild matching efforts for one saved segment across existing
          activities. Use this after changing segment matching thresholds or
          when one trail needs fresh effort rows without rerunning every stored
          activity import.
        </p>

        <label className="form-control mt-5 max-w-sm">
          <div className="label">
            <span className="label-text font-medium">Segment id</span>
            <span className="label-text-alt">Required</span>
          </div>
          <input
            type="number"
            min={1}
            step={1}
            inputMode="numeric"
            className="input input-bordered"
            placeholder="51"
            value={segmentEffortSegmentId}
            onChange={(event) => {
              setSegmentEffortSegmentId(event.target.value);
            }}
          />
          <div className="label">
            <span className="label-text-alt text-base-content/60">
              This queues a targeted worker task and refreshes analytics for
              the affected segment.
            </span>
          </div>
        </label>

        <div className="mt-5 flex flex-wrap gap-3">
          <button
            className="btn btn-primary"
            disabled={
              !segmentEffortSegmentId.trim() ||
              isSegmentEffortRegenerationPending
            }
            onClick={handleSegmentEffortRegeneration}
          >
            {isSegmentEffortRegenerationPending
              ? "Queueing regeneration..."
              : "Regenerate segment efforts"}
          </button>
          <Link href="/admin/tasks" className="btn btn-ghost">
            View background tasks
          </Link>
        </div>

        {segmentEffortErrorMessage ? (
          <div className="alert alert-error mt-4">
            <span>{segmentEffortErrorMessage}</span>
          </div>
        ) : null}

        {segmentEffortResult ? (
          <div className="mt-5 rounded-2xl bg-base-200 p-5">
            <div className="text-sm text-base-content/60">
              Segment effort request
            </div>
            <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2 xl:grid-cols-3">
              <SummaryItem
                label="Segment id"
                value={segmentEffortResult.segment_id}
              />
              <SummaryTextItem
                label="Status"
                value={segmentEffortResult.status}
              />
              <SummaryTextItem
                label="Message"
                value={segmentEffortResult.message}
              />
            </dl>
          </div>
        ) : null}
      </section>

      <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
        <h2 className="text-lg font-semibold">Regenerate user segments</h2>
        <p className="mt-2 max-w-3xl text-sm text-base-content/70">
          Rebuild segment matching for one rider from stored activities. This
          is broader than the targeted segment tool and should be used when one
          rider needs all effort matches refreshed.
        </p>

        <label className="form-control mt-5 max-w-sm">
          <div className="label">
            <span className="label-text font-medium">User id</span>
            <span className="label-text-alt">Required</span>
          </div>
          <input
            type="number"
            min={1}
            step={1}
            inputMode="numeric"
            className="input input-bordered"
            placeholder="42"
            value={userSegmentUserId}
            onChange={(event) => {
              setUserSegmentUserId(event.target.value);
            }}
          />
        </label>

        <div className="mt-5 flex flex-wrap gap-3">
          <button
            className="btn btn-primary"
            disabled={
              !userSegmentUserId.trim() || isUserSegmentRegenerationPending
            }
            onClick={handleUserSegmentRegeneration}
          >
            {isUserSegmentRegenerationPending
              ? "Queueing regeneration..."
              : "Regenerate user segments"}
          </button>
          <Link href="/admin/users" className="btn btn-ghost">
            Find user ids
          </Link>
        </div>

        {userSegmentErrorMessage ? (
          <div className="alert alert-error mt-4">
            <span>{userSegmentErrorMessage}</span>
          </div>
        ) : null}

        {userSegmentResult ? (
          <div className="mt-5 rounded-2xl bg-base-200 p-5">
            <div className="text-sm text-base-content/60">
              Segment regeneration request
            </div>
            <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2 xl:grid-cols-3">
              <SummaryItem label="User id" value={userSegmentResult.user_id} />
              <SummaryTextItem
                label="Status"
                value={userSegmentResult.status}
              />
              <SummaryTextItem
                label="Message"
                value={userSegmentResult.message}
              />
            </dl>
          </div>
        ) : null}
      </section>

      <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
        <h2 className="text-lg font-semibold">
          Fetch provider export archives
        </h2>
        <p className="mt-2 max-w-3xl text-sm text-base-content/70">
          Paste a shareable Garmin Connect or Strava export URL and Bike will
          queue a worker task that fetches the archive server-side, imports
          supported activity files, and deduplicates anything already stored for
          that rider.
        </p>

        <label className="form-control mt-5">
          <div className="label">
            <span className="label-text font-medium">Archive URL</span>
            <span className="label-text-alt">HTTPS export link</span>
          </div>
          <input
            type="url"
            className="input input-bordered"
            placeholder="https://.../export.zip"
            value={archiveUrl}
            onChange={(event) => {
              setArchiveUrl(event.target.value);
            }}
          />
          <div className="label">
            <span className="label-text-alt text-base-content/60">
              Bike fetches the archive directly, so large exports do not go
              through the browser upload path.
            </span>
          </div>
        </label>

        <div className="mt-5 flex flex-wrap gap-3">
          <button
            className="btn btn-primary"
            disabled={!archiveUrl.trim() || isImportPending}
            onClick={handleArchiveImport}
          >
            {isImportPending ? "Queueing import..." : "Queue archive import"}
          </button>
          <Link href="/admin/tasks" className="btn btn-ghost">
            View background tasks
          </Link>
        </div>

        {archiveErrorMessage ? (
          <div className="alert alert-error mt-4">
            <span>{archiveErrorMessage}</span>
          </div>
        ) : null}

        {archiveResult ? (
          <div className="mt-5 grid gap-4 lg:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)]">
            <div className="rounded-2xl bg-base-200 p-5">
              <div className="text-sm text-base-content/60">
                Queued archive job
              </div>
              <div className="mt-2 break-all text-base font-medium">
                {archiveResult.archive_url}
              </div>
              <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2">
                <SummaryItem label="Job id" value={archiveResult.id} />
                <SummaryItem
                  label="Imported activities"
                  value={archiveResult.imported_count}
                />
                <SummaryItem
                  label="Duplicates skipped"
                  value={archiveResult.duplicate_count}
                />
                <SummaryItem
                  label="Failed entries"
                  value={archiveResult.failed_count}
                />
              </dl>
            </div>

            <div className="rounded-2xl bg-base-200 p-5">
              <div className="text-sm text-base-content/60">Job status</div>
              <div className="mt-2 text-3xl font-semibold uppercase">
                {archiveResult.status}
              </div>
              <p className="mt-2 text-sm text-base-content/70">
                Refreshes automatically while the worker is running. The request
                only queues the import now.
              </p>
              {archiveResult.failure_message ? (
                <div className="mt-4 rounded-xl border border-base-300 bg-base-100 px-3 py-2 text-sm text-base-content/70">
                  {archiveResult.failure_message}
                </div>
              ) : archiveResult.error_samples.length > 0 ? (
                <div className="mt-4 space-y-2 text-sm text-base-content/80">
                  {archiveResult.error_samples.map((sample) => (
                    <div
                      key={sample}
                      className="rounded-xl border border-base-300 bg-base-100 px-3 py-2"
                    >
                      {sample}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-4 rounded-xl border border-base-300 bg-base-100 px-3 py-2 text-sm text-base-content/70">
                  No failure details yet.
                </div>
              )}
            </div>
          </div>
        ) : null}

        {archiveJobsQuery.data.length > 0 ? (
          <div className="mt-5 rounded-2xl bg-base-200 p-5">
            <div className="text-sm text-base-content/60">
              Recent archive jobs
            </div>
            <div className="mt-4 space-y-2 text-sm">
              {archiveJobsQuery.data.map((job) => (
                <div
                  key={job.id}
                  className="rounded-xl border border-base-300 bg-base-100 px-3 py-2"
                >
                  <div className="font-medium uppercase">{job.status}</div>
                  <div className="break-all text-base-content/70">
                    {job.resolved_url ?? job.archive_url}
                  </div>
                </div>
              ))}
            </div>
          </div>
        ) : null}
      </section>

      <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
        <h2 className="text-lg font-semibold">Clean up duplicate activities</h2>
        <p className="mt-2 max-w-3xl text-sm text-base-content/70">
          Scan one rider&apos;s stored activities using the current duplicate
          matcher, keep the preferred copy for each duplicate cluster, delete
          the extra activities, and queue cache rebuilds for any affected
          segments and fitness history.
        </p>

        <label className="form-control mt-5 max-w-sm">
          <div className="label">
            <span className="label-text font-medium">User id</span>
            <span className="label-text-alt">Required</span>
          </div>
          <input
            type="number"
            min={1}
            step={1}
            inputMode="numeric"
            className="input input-bordered"
            placeholder="42"
            value={cleanupUserId}
            onChange={(event) => {
              setCleanupUserId(event.target.value);
            }}
          />
          <div className="label">
            <span className="label-text-alt text-base-content/60">
              Cleanup prefers richer source files like FIT, then denser route
              detail, then older records when choosing which copy to keep.
            </span>
          </div>
        </label>

        <div className="mt-5 flex flex-wrap gap-3">
          <button
            className="btn btn-primary"
            disabled={!cleanupUserId.trim() || isCleanupPending}
            onClick={handleCleanupDuplicates}
          >
            {isCleanupPending ? "Cleaning duplicates..." : "Clean duplicates"}
          </button>
          <Link href="/admin/users" className="btn btn-ghost">
            Find user ids
          </Link>
        </div>

        {cleanupErrorMessage ? (
          <div className="alert alert-error mt-4">
            <span>{cleanupErrorMessage}</span>
          </div>
        ) : null}

        {cleanupResult ? (
          <div className="mt-5 rounded-2xl bg-base-200 p-5">
            <div className="text-sm text-base-content/60">Cleanup result</div>
            <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2 xl:grid-cols-3">
              <SummaryItem label="User id" value={cleanupResult.user_id} />
              <SummaryTextItem label="Status" value={cleanupResult.status} />
              <SummaryItem
                label="Duplicate groups"
                value={cleanupResult.duplicate_group_count}
              />
              <SummaryItem
                label="Deleted activities"
                value={cleanupResult.deleted_activity_count}
              />
              <SummaryItem
                label="Retained activities"
                value={cleanupResult.retained_activity_count}
              />
            </dl>
            <div className="mt-4 rounded-xl border border-base-300 bg-base-100 px-4 py-3 text-sm text-base-content/80">
              {cleanupResult.message}
            </div>
          </div>
        ) : null}
      </section>

      <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
        <h2 className="text-lg font-semibold">Backfill XC training history</h2>
        <p className="mt-2 max-w-3xl text-sm text-base-content/70">
          Queue the XC training-history backfill for one rider. This rebuilds
          cached historical training metrics from the stored activity records
          without rerunning full activity import reprocessing, and is the same
          workflow the XC page now requests automatically when a start date
          changes.
        </p>

        <label className="form-control mt-5 max-w-sm">
          <div className="label">
            <span className="label-text font-medium">User id</span>
            <span className="label-text-alt">Required</span>
          </div>
          <input
            type="number"
            min={1}
            step={1}
            inputMode="numeric"
            className="input input-bordered"
            placeholder="42"
            value={xcBackfillUserId}
            onChange={(event) => {
              setXcBackfillUserId(event.target.value);
            }}
          />
          <div className="label">
            <span className="label-text-alt text-base-content/60">
              Use this when one rider needs historical XC trend data rebuilt.
            </span>
          </div>
        </label>

        <div className="mt-5 flex flex-wrap gap-3">
          <button
            className="btn btn-primary"
            disabled={!xcBackfillUserId.trim() || isXcBackfillPending}
            onClick={handleXcTrainingBackfill}
          >
            {isXcBackfillPending
              ? "Queueing XC backfill..."
              : "Queue XC backfill"}
          </button>
          <Link href="/admin/users" className="btn btn-ghost">
            Find user ids
          </Link>
          <Link href="/admin/tasks" className="btn btn-ghost">
            View background tasks
          </Link>
        </div>

        {xcBackfillErrorMessage ? (
          <div className="alert alert-error mt-4">
            <span>{xcBackfillErrorMessage}</span>
          </div>
        ) : null}

        {xcBackfillResult ? (
          <div className="mt-5 rounded-2xl bg-base-200 p-5">
            <div className="text-sm text-base-content/60">
              XC backfill request
            </div>
            <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2 xl:grid-cols-3">
              <SummaryItem label="User id" value={xcBackfillResult.user_id} />
              <SummaryTextItem label="Status" value={xcBackfillResult.status} />
              <SummaryTextItem
                label="Message"
                value={xcBackfillResult.message}
              />
            </dl>
          </div>
        ) : null}
      </section>

      <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
        <h2 className="text-lg font-semibold">
          Reprocess imported activity files
        </h2>
        <p className="mt-2 max-w-3xl text-sm text-base-content/70">
          Queue a stored-file reprocess for one rider. This reuses the same
          per-activity import files created by archive imports and reruns the
          normal import pipeline instead of taking a separate admin-only path.
        </p>

        <label className="form-control mt-5 max-w-sm">
          <div className="label">
            <span className="label-text font-medium">User id</span>
            <span className="label-text-alt">Required</span>
          </div>
          <input
            type="number"
            min={1}
            step={1}
            inputMode="numeric"
            className="input input-bordered"
            placeholder="42"
            value={reprocessUserId}
            onChange={(event) => {
              setReprocessUserId(event.target.value);
            }}
          />
          <div className="label">
            <span className="label-text-alt text-base-content/60">
              Use the admin users page if you need to look up a rider id.
            </span>
          </div>
        </label>

        <div className="mt-5 flex flex-wrap gap-3">
          <button
            className="btn btn-primary"
            disabled={!reprocessUserId.trim() || isReprocessPending}
            onClick={handleReprocessImports}
          >
            {isReprocessPending
              ? "Queueing reprocess..."
              : "Queue import reprocess"}
          </button>
          <Link href="/admin/users" className="btn btn-ghost">
            Find user ids
          </Link>
          <Link href="/admin/tasks" className="btn btn-ghost">
            View background tasks
          </Link>
        </div>

        {reprocessErrorMessage ? (
          <div className="alert alert-error mt-4">
            <span>{reprocessErrorMessage}</span>
          </div>
        ) : null}

        {reprocessResult ? (
          <div className="mt-5 rounded-2xl bg-base-200 p-5">
            <div className="text-sm text-base-content/60">Queued reprocess</div>
            <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2 xl:grid-cols-3">
              <SummaryItem label="User id" value={reprocessResult.user_id} />
              <SummaryTextItem label="Status" value={reprocessResult.status} />
              <SummaryTextItem
                label="Message"
                value={reprocessResult.message}
              />
            </dl>
          </div>
        ) : null}
      </section>

      <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
        <h2 className="text-lg font-semibold">Prewarm analytics caches</h2>
        <p className="mt-2 max-w-2xl text-sm text-base-content/70">
          Queue a one-off rebuild for every rider with activities and every
          saved segment so fitness, fatigue, form, segment PRs, and leaderboard
          summaries are ready before the next read.
        </p>

        <div className="mt-5 flex flex-wrap gap-3">
          <button
            className="btn btn-primary"
            disabled={isPending}
            onClick={handleBackfill}
          >
            {isPending ? "Enqueuing..." : "Queue analytics backfill"}
          </button>
          <Link href="/admin/tasks" className="btn btn-ghost">
            View background tasks
          </Link>
        </div>

        {errorMessage ? (
          <div className="alert alert-error mt-4">
            <span>{errorMessage}</span>
          </div>
        ) : null}
      </section>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        <MetricCard
          label="Fitness rebuilds"
          value={result?.fitness_task_count ?? 0}
          helper="One task per rider with existing activity history"
        />
        <MetricCard
          label="Segment rebuilds"
          value={result?.segment_task_count ?? 0}
          helper="Chunked worker jobs for saved segments"
        />
        <MetricCard
          label="Total tasks"
          value={result?.total_tasks_enqueued ?? 0}
          helper="Background jobs enqueued by the last run"
        />
      </section>

      <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
        <h3 className="text-base font-semibold">Last enqueue summary</h3>
        <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2 xl:grid-cols-4">
          <SummaryItem label="Users queued" value={result?.user_count ?? 0} />
          <SummaryItem
            label="Segments queued"
            value={result?.segment_count ?? 0}
          />
          <SummaryItem
            label="Segment chunk size"
            value={result?.segment_chunk_size ?? 250}
          />
          <SummaryItem
            label="Task batches"
            value={result?.total_tasks_enqueued ?? 0}
          />
        </dl>
        <p className="mt-4 text-sm text-base-content/70">
          The backfill is safe to rerun. It only enqueues rebuilds against the
          base tables, so uploads, deletes, and regenerations continue to
          converge on the same cached results.
        </p>
      </section>
    </div>
  );
}

function MetricCard({
  label,
  value,
  helper,
}: {
  label: string;
  value: number;
  helper: string;
}) {
  return (
    <div className="rounded-2xl border border-base-300 bg-base-100 p-5 shadow-sm">
      <div className="text-sm text-base-content/60">{label}</div>
      <div className="mt-2 text-3xl font-semibold">{value}</div>
      <p className="mt-2 text-sm text-base-content/70">{helper}</p>
    </div>
  );
}

function SummaryItem({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-xl bg-base-200 px-4 py-3">
      <dt className="text-base-content/60">{label}</dt>
      <dd className="mt-1 text-lg font-semibold">{value}</dd>
    </div>
  );
}

function SummaryTextItem({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl bg-base-200 px-4 py-3">
      <dt className="text-base-content/60">{label}</dt>
      <dd className="mt-1 break-words text-lg font-semibold">{value}</dd>
    </div>
  );
}
