"use client";

import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";
import AdminTaskHistory from "../../../components/admin/AdminTaskHistory";

export default function AdminTasksPage() {
  return (
    <LocalAdminLayout title="Tasks">
      <AdminTaskHistory />
    </LocalAdminLayout>
  );
}
