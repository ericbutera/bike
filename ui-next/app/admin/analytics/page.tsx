"use client";

import {
  useAdminBackfillAnalytics,
  type AdminAnalyticsBackfillResponse,
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
  const [result, setResult] = useState<AdminAnalyticsBackfillResponse | null>(
    null,
  );
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

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

  return (
    <div className="grid gap-6 p-6">
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
