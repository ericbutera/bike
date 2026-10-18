"use client";

import { useAdminUser, type AdminUser } from "../../lib/queries";
import { formatActivityTimestamp } from "../../lib/activityFormatting";
import Modal from "../ui/Modal";
import { ProcessingFacts } from "../ui/ProcessingDetails";
import StatusBadge from "../ui/StatusBadge";

export default function AdminUserDetailsDialog({
  user,
  onClose,
}: {
  user: AdminUser;
  onClose: () => void;
}) {
  const query = useAdminUser(user.id);
  const detail = query.data;
  return (
    <Modal title={`User #${user.id} details`} onClose={onClose}>
      {query.isLoading ? <p className="my-4">Loading user details...</p> : null}
      {query.error ? (
        <p role="alert" className="my-4 text-error">
          Unable to load user details.
        </p>
      ) : null}
      {detail ? (
        <div className="my-4">
          <ProcessingFacts
            facts={[
              { label: "Name", value: detail.name || "—" },
              { label: "Email", value: detail.email },
              {
                label: "Status",
                value: (
                  <StatusBadge
                    status={detail.disabled ? "Disabled" : "Active"}
                  />
                ),
              },
              {
                label: "Verified",
                value: (
                  <StatusBadge
                    status={detail.email_verified ? "Verified" : "Unverified"}
                  />
                ),
              },
              { label: "Admin user", value: detail.is_admin ? "Yes" : "No" },
              {
                label: "Created",
                value: formatActivityTimestamp(detail.created_at),
              },
              {
                label: "Last login",
                value: detail.last_login_at
                  ? formatActivityTimestamp(detail.last_login_at)
                  : "—",
              },
            ]}
          />
        </div>
      ) : null}
    </Modal>
  );
}
