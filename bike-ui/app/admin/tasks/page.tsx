"use client";

import { Suspense } from "react";
import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";
import TasksPageContent from "../../../components/admin/TasksPageContent";

export default function AdminTasksPage() {
  return (
    <Suspense>
      <LocalAdminLayout title="Tasks">
        <TasksPageContent />
      </LocalAdminLayout>
    </Suspense>
  );
}
