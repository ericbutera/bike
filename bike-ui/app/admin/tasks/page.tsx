"use client";

import { LocalAdminLayout } from "../../../components/admin/LocalAdminLayout";
import { AppCard } from "../../../components/ui/Card";
import TemporalLink from "../../../components/ui/TemporalLink";
import Link from "next/link";
import AdminTaskHistory from "../../../components/admin/AdminTaskHistory";

export default function AdminTasksPage() {
  return (
    <LocalAdminLayout title="Tasks">
      <AppCard className="mb-6">
        <h2 className="card-title">Background tasks</h2>
        <p>
          View recent runs below. Open Temporal for detailed history and
          execution controls.
        </p>
        <TemporalLink />
        <p>Run activity and user tasks from the gear menu on their rows.</p>
        <div className="flex flex-wrap gap-4">
          <Link className="link" href="/admin/activities">
            Activity tasks
          </Link>
          <Link className="link" href="/admin/users">
            User tasks
          </Link>
          <Link className="link" href="/admin/manual-tasks">
            Other maintenance tools
          </Link>
        </div>
      </AppCard>
      <AdminTaskHistory />
    </LocalAdminLayout>
  );
}
