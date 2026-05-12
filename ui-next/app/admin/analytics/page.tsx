"use client";

import { extractApiMessage } from "@/lib/activityFormatting";
import {
  useActivityArchiveImportJobs,
  useAdminBackfillAnalytics,
  useImportActivityArchiveUrl,
  useReprocessUserActivityImports,
  type ActivityArchiveImportJob,
  type AdminAnalyticsBackfillResponse,
  type ReprocessUserActivityImportsResponse,
} from "@/lib/queries";
import { admin } from "@ericbutera/kaleido";
import Link from "next/link";
import { Suspense, useState } from "react";
import AuthRouter from "../../../components/AuthRouter";

export default function AdminAnalyticsPage() {
  return (
    <Suspense>
      <AuthRouter>
        <admin.Layout title="Analytics">
          <AnalyticsContent />
        </admin.Layout>
      </AuthRouter>
    </Suspense>
  );
}

function AnalyticsContent() {
  const { backfillAsync, isPending } = useAdminBackfillAnalytics();
  const archiveJobsQuery = useActivityArchiveImportJobs({
    enabled: true,
    refetchIntervalMs: 5000,
  });
  const { importAsync, isPending: isImportPending } =
    useImportActivityArchiveUrl();
  const { reprocessAsync, isPending: isReprocessPending } =
    useReprocessUserActivityImports();
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

  return (
    <div className="grid gap-6 p-6">
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
