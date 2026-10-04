"use client";

import BikeMetricsSection from "@/components/admin/BikeMetricsSection";
import { useAdminAppMetrics, useAdminMetrics } from "@/lib/queries";
import { Suspense } from "react";
import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";

export default function AdminMetricsPage() {
  return (
    <Suspense>
      <LocalAdminLayout title="Metrics">
        <MetricsContent />
      </LocalAdminLayout>
    </Suspense>
  );
}

function MetricsContent() {
  const { data: systemData, isLoading: sysLoading } = useAdminMetrics();
  const { data: appData, isLoading: appLoading } = useAdminAppMetrics();

  if (sysLoading || appLoading) return <div className="p-6">Loading...</div>;

  return (
    <>
      {Object.entries(systemData ?? {}).map(([section, stats]) => (
        <BikeMetricsSection
          key={section}
          title={METRIC_SECTION_LABELS[section] ?? section.replaceAll("_", " ")}
          stats={stats}
        />
      ))}
      <BikeMetricsSection stats={appData} />
    </>
  );
}

const METRIC_SECTION_LABELS: Record<string, string> = {
  auth: "Auth",
  background_tasks: "Background Tasks",
};
