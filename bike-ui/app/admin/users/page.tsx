"use client";

import { Suspense } from "react";
import AdminUsersPageContent from "../../../components/admin/AdminUsersPageContent";
import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";

export default function AdminUsersPage() {
  return (
    <Suspense>
      <LocalAdminLayout title="Dashboard">
        <AdminUsersPageContent />
      </LocalAdminLayout>
    </Suspense>
  );
}
