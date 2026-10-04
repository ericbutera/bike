"use client";

import { Suspense } from "react";
import FeatureFlagsPanel from "../../../components/admin/FeatureFlagsPanel";
import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";

export default function FeatureFlagsPage() {
  return (
    <Suspense>
      <LocalAdminLayout title="Dashboard">
        <FeatureFlagsPanel />
      </LocalAdminLayout>
    </Suspense>
  );
}
