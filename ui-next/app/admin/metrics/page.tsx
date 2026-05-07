"use client";

import { useAdminAppMetrics, useAdminMetrics } from "@/lib/queries";
import { admin } from "@ericbutera/kaleido";
import { Suspense } from "react";
import AuthRouter from "../../../components/AuthRouter";

export default function AdminMetricsPage() {
  return (
    <Suspense>
      <AuthRouter>
        <admin.Layout title="Metrics">
          <MetricsContent />
        </admin.Layout>
      </AuthRouter>
    </Suspense>
  );
}

function MetricsContent() {
  const { data: sysData, isLoading: sysLoading } = useAdminMetrics();
  const { data: appData, isLoading: appLoading } = useAdminAppMetrics();

  if (sysLoading || appLoading) return <div className="p-6">Loading...</div>;

  return (
    <div className="grid gap-6 p-6">
      <section className="rounded-2xl border border-base-300 bg-base-100 p-4 shadow-sm">
        <h2 className="text-lg font-semibold">System metrics</h2>
        <pre className="mt-4 overflow-x-auto rounded-xl bg-base-200 p-4 text-xs">
          {JSON.stringify(sysData, null, 2)}
        </pre>
      </section>

      <section className="rounded-2xl border border-base-300 bg-base-100 p-4 shadow-sm">
        <h2 className="text-lg font-semibold">Application metrics</h2>
        <pre className="mt-4 overflow-x-auto rounded-xl bg-base-200 p-4 text-xs">
          {JSON.stringify(appData, null, 2)}
        </pre>
      </section>
    </div>
  );
}
