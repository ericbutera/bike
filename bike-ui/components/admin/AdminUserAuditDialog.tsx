"use client";

import { useState } from "react";
import { useAdminUserAuditEvents, type AdminUser } from "../../lib/queries";
import { formatActivityTimestamp } from "../../lib/activityFormatting";
import Modal from "../ui/Modal";
import { DataGrid, type Column } from "../ui/DataGrid";
import Pagination from "../ui/Pagination";

type AuditEvent = NonNullable<
  ReturnType<typeof useAdminUserAuditEvents>["data"]
>["data"][number];

const columns: Column<AuditEvent>[] = [
  {
    key: "event",
    header: "Event",
    render: (event) => event.event_type.replaceAll("_", " "),
  },
  { key: "ip", header: "IP address", render: (event) => event.ip || "—" },
  {
    key: "agent",
    header: "User agent",
    className: "max-w-xs truncate",
    render: (event) => (
      <span title={event.user_agent ?? undefined}>
        {event.user_agent || "—"}
      </span>
    ),
  },
  {
    key: "reason",
    header: "Reason",
    className: "max-w-xs truncate",
    render: (event) => (
      <span title={event.reason ?? undefined}>{event.reason || "—"}</span>
    ),
  },
  {
    key: "occurred",
    header: "Occurred",
    className: "whitespace-nowrap",
    render: (event) => formatActivityTimestamp(event.occurred_at),
  },
];

export default function AdminUserAuditDialog({
  user,
  onClose,
}: {
  user: AdminUser;
  onClose: () => void;
}) {
  const [page, setPage] = useState(1);
  const query = useAdminUserAuditEvents(user.id, page);
  const metadata = query.data?.metadata;
  return (
    <Modal
      title={`Audit events for ${user.name || user.email} (#${user.id})`}
      onClose={onClose}
      className="max-w-5xl"
    >
      <div className="mt-4">
        <DataGrid
          rows={query.data?.data ?? []}
          columns={columns}
          isLoading={query.isLoading}
          error={query.error ? "Unable to load audit events." : undefined}
          emptyMessage="No audit events recorded for this user."
          label="User audit events"
        />
        {metadata ? (
          <Pagination
            className="mt-4"
            page={page}
            perPage={metadata.per_page}
            total={metadata.total}
            onPageChange={setPage}
            disabled={query.isFetching}
          />
        ) : null}
      </div>
    </Modal>
  );
}
