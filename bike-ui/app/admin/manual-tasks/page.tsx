"use client";

import AdminTaskTools from "@/components/admin/AdminTaskTools";
import { Suspense } from "react";
import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";

export default function AdminManualTasksPage() {
  return (
    <Suspense>
      <LocalAdminLayout title="Manual tasks">
        <div className="p-6">
          <AdminTaskTools />
        </div>
      </LocalAdminLayout>
    </Suspense>
  );
}
