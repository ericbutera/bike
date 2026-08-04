"use client";

import {
  useActivityArchiveImportJobs,
  useAdminBackfillAnalytics,
  useAdminBackfillUserXcTraining,
  useCleanupUserDuplicateActivities,
  useImportActivityArchiveUrl,
  useRegenerateSegmentEfforts,
  useRegenerateUserSegments,
  useReprocessActivityImport,
  useReprocessUserActivityImports,
  type ActivityArchiveImportJob,
  type AdminAnalyticsBackfillResponse,
  type CleanupUserDuplicateActivitiesResponse,
  type RegenerateSegmentEffortsResponse,
  type RegenerateUserSegmentsResponse,
  type ReprocessActivityImportResponse,
  type ReprocessUserActivityImportsResponse,
} from "@/lib/queries";
import Link from "next/link";
import { useState } from "react";
import toast from "react-hot-toast";

export default function AdminTaskTools() {
  const { backfillAsync, isPending } = useAdminBackfillAnalytics();
  const archiveJobsQuery = useActivityArchiveImportJobs({
    enabled: true,
    refetchIntervalMs: 5000,
  });
  const archiveJobs = archiveJobsQuery.data ?? [];
  const { importAsync, isPending: isImportPending } =
    useImportActivityArchiveUrl();
  const { cleanupAsync, isPending: isCleanupPending } =
    useCleanupUserDuplicateActivities();
  const { reprocessAsync, isPending: isReprocessPending } =
    useReprocessUserActivityImports();
  const {
    reprocessAsync: reprocessActivityAsync,
    isPending: isActivityReprocessPending,
  } = useReprocessActivityImport();
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
  const [archiveUrl, setArchiveUrl] = useState("");
  const [archiveResult, setArchiveResult] =
    useState<ActivityArchiveImportJob | null>(null);
  const [reprocessUserId, setReprocessUserId] = useState("");
  const [reprocessResult, setReprocessResult] =
    useState<ReprocessUserActivityImportsResponse | null>(null);
  const [activityReprocessId, setActivityReprocessId] = useState("");
  const [activityReprocessResult, setActivityReprocessResult] =
    useState<ReprocessActivityImportResponse | null>(null);
  const [userSegmentUserId, setUserSegmentUserId] = useState("");
  const [userSegmentResult, setUserSegmentResult] =
    useState<RegenerateUserSegmentsResponse | null>(null);
  const [segmentEffortSegmentId, setSegmentEffortSegmentId] = useState("");
  const [segmentEffortResult, setSegmentEffortResult] =
    useState<RegenerateSegmentEffortsResponse | null>(null);
  const [xcBackfillUserId, setXcBackfillUserId] = useState("");
  const [xcBackfillResult, setXcBackfillResult] =
    useState<ReprocessUserActivityImportsResponse | null>(null);
  const [cleanupUserId, setCleanupUserId] = useState("");
  const [cleanupResult, setCleanupResult] =
    useState<CleanupUserDuplicateActivitiesResponse | null>(null);

  async function handleBackfill() {
    try {
      const response = await backfillAsync();
      setResult(response);
    } catch {
      setResult(null);
    }
  }

  async function handleArchiveImport() {
    try {
      const response = await importAsync(archiveUrl.trim());
      setArchiveResult(response);
    } catch {
      setArchiveResult(null);
    }
  }

  async function handleReprocessImports() {
    const numericUserId = Number(reprocessUserId);

    if (!Number.isFinite(numericUserId) || numericUserId < 1) {
      setReprocessResult(null);
      toast.error("Enter a valid user id.");
      return;
    }

    try {
      const response = await reprocessAsync(numericUserId);
      setReprocessResult(response);
    } catch {
      setReprocessResult(null);
    }
  }

  async function handleActivityReprocess() {
    const numericActivityId = Number(activityReprocessId);

    if (!Number.isFinite(numericActivityId) || numericActivityId < 1) {
      setActivityReprocessResult(null);
      toast.error("Enter a valid activity id.");
      return;
    }

    try {
      const response = await reprocessActivityAsync(numericActivityId);
      setActivityReprocessResult(response);
    } catch {
      setActivityReprocessResult(null);
    }
  }

  async function handleUserSegmentRegeneration() {
    const numericUserId = Number(userSegmentUserId);

    if (!Number.isFinite(numericUserId) || numericUserId < 1) {
      setUserSegmentResult(null);
      toast.error("Enter a valid user id.");
      return;
    }

    try {
      const response = await regenerateUserSegmentsAsync(numericUserId);
      setUserSegmentResult(response);
    } catch {
      setUserSegmentResult(null);
    }
  }

  async function handleSegmentEffortRegeneration() {
    const numericSegmentId = Number(segmentEffortSegmentId);

    if (!Number.isFinite(numericSegmentId) || numericSegmentId < 1) {
      setSegmentEffortResult(null);
      toast.error("Enter a valid segment id.");
      return;
    }

    try {
      const response = await regenerateSegmentEffortsAsync(numericSegmentId);
      setSegmentEffortResult(response);
    } catch {
      setSegmentEffortResult(null);
    }
  }

  async function handleCleanupDuplicates() {
    const numericUserId = Number(cleanupUserId);

    if (!Number.isFinite(numericUserId) || numericUserId < 1) {
      setCleanupResult(null);
      toast.error("Enter a valid user id.");
      return;
    }

    try {
      const response = await cleanupAsync(numericUserId);
      setCleanupResult(response);
    } catch {
      setCleanupResult(null);
    }
  }

  async function handleXcTrainingBackfill() {
    const numericUserId = Number(xcBackfillUserId);

    if (!Number.isFinite(numericUserId) || numericUserId < 1) {
      setXcBackfillResult(null);
      toast.error("Enter a valid user id.");
      return;
    }

    try {
      const response = await backfillXcTrainingAsync(numericUserId);
      setXcBackfillResult(response);
    } catch {
      setXcBackfillResult(null);
    }
  }

  return (
    <div className="grid gap-6">
      <section className="rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
        <h2 className="text-lg font-semibold">
          Reprocess one imported activity
        </h2>
        <p className="mt-2 max-w-3xl text-sm text-base-content/70">
          Queue a focused rebuild for one activity from its stored source file.
          This refreshes segment efforts, segment analytics, activity analytics,
          XC training analysis, and fitness freshness without rerunning every
          activity for the rider.
        </p>

        <label className="form-control mt-5 max-w-sm">
          <div className="label">
            <span className="label-text font-medium">Activity id</span>
            <span className="label-text-alt">Required</span>
          </div>
          <input
            type="number"
            min={1}
            step={1}
            inputMode="numeric"
            className="input input-bordered"
            placeholder="1647"
            value={activityReprocessId}
            onChange={(event) => {
              setActivityReprocessId(event.target.value);
            }}
          />
          <div className="label">
            <span className="label-text-alt text-base-content/60">
              The activity must be linked to a stored import file.
            </span>
          </div>
        </label>

        <div className="mt-5 flex flex-wrap gap-3">
          <button
            className="btn btn-primary"
            disabled={
              !activityReprocessId.trim() || isActivityReprocessPending
            }
            onClick={handleActivityReprocess}
          >
            {isActivityReprocessPending
              ? "Queueing reprocess..."
              : "Queue activity reprocess"}
          </button>
          <Link href="/admin/tasks" className="btn btn-ghost">
            View background tasks
          </Link>
          <Link href="/admin/integrations" className="btn btn-ghost">
            View processing events
          </Link>
        </div>

        {activityReprocessResult ? (
          <div className="mt-5 rounded-2xl bg-base-200 p-5">
            <div className="text-sm text-base-content/60">
              Queued activity reprocess
            </div>
            <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2 xl:grid-cols-3">
              <SummaryItem
                label="Activity id"
                value={activityReprocessResult.activity_id}
              />
              <SummaryItem
                label="Import id"
                value={activityReprocessResult.activity_import_id}
              />
              <SummaryItem
                label="User id"
                value={activityReprocessResult.user_id}
              />
              <SummaryTextItem
                label="Task id"
                value={activityReprocessResult.task_id}
              />
              <SummaryTextItem
                label="Status"
                value={activityReprocessResult.status}
              />
              <SummaryTextItem
                label="Message"
                value={activityReprocessResult.message}
              />
            </dl>
            <div className="mt-5 flex flex-wrap gap-3">
              <Link
                href={`/admin/integrations?provider=activity_processing&user_id=${activityReprocessResult.user_id}&activity_id=${activityReprocessResult.activity_id}`}
                className="btn btn-sm btn-outline"
              >
                Trace activity events
              </Link>
              <Link
                href={`/admin/integrations?provider=activity_processing&user_id=${activityReprocessResult.user_id}&import_id=${activityReprocessResult.activity_import_id}`}
                className="btn btn-sm btn-outline"
              >
                Trace import events
              </Link>
              <Link href="/admin/tasks" className="btn btn-sm btn-ghost">
                View task queue
              </Link>
            </div>
          </div>
        ) : null}
      </section>

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

        {archiveJobs.length > 0 ? (
          <div className="mt-5 rounded-2xl bg-base-200 p-5">
            <div className="text-sm text-base-content/60">
              Recent archive jobs
            </div>
            <div className="mt-4 space-y-2 text-sm">
              {archiveJobs.map((job) => (
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
