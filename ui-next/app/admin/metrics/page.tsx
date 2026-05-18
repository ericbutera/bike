"use client";

import BikeMetricsSection from "@/components/admin/BikeMetricsSection";
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
    <>
      <admin.KaleidoMetricsSection data={sysData} />
      <BikeMetricsSection stats={appData} />
    </>
  );
}
