"use client";

import { Suspense } from "react";
import { useSearchParams } from "next/navigation";
import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";
import TasksPageContent from "../../../components/admin/TasksPageContent";

export default function AdminTasksPage() {
  return (
    <Suspense>
      <LocalAdminLayout title="Tasks">
        <LinkedTasksPage />
      </LocalAdminLayout>
    </Suspense>
  );
}

function LinkedTasksPage() {
  const params = useSearchParams();
  return (
    <TasksPageContent
      initialCorrelationId={params.get("correlation_id") ?? ""}
    />
  );
}
