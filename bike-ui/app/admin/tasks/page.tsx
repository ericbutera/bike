"use client";

import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";
import { AppCard } from "../../../components/ui/Card";
import TemporalLink from "../../../components/ui/TemporalLink";

export default function AdminTasksPage() {
  return (
    <LocalAdminLayout title="Tasks">
      <AppCard>
        <h2 className="card-title">Background tasks</h2>
        <p>
          Inspect running tasks, failures, retries, and cancellations in
          Temporal.
        </p>
        <TemporalLink />
      </AppCard>
    </LocalAdminLayout>
  );
}
