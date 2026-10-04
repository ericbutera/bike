"use client";

import { Suspense } from "react";
import { LocalAdminLayout } from "../../components/admin/LocalAdminLayout";

export default function AdminPage() {
  return (
    <Suspense>
      <LocalAdminLayout title="Dashboard">
        <div>Admin panel</div>
      </LocalAdminLayout>
    </Suspense>
  );
}
