"use client";

import { useMemo } from "react";
import GenericList, { type Column } from "../ui/GenericList";
import { useAdminUsers, type AdminUser } from "../../lib/queries";
import { UserTaskMenu } from "./AdminTaskMenu";
import StatusBadge from "../ui/StatusBadge";
import { formatActivityTimestamp } from "../../lib/activityFormatting";

type AdminUsersGridParams = {
  q?: string;
  disabled?: "true" | "false";
  page?: number | string;
  per_page?: number | string;
};

const ADMIN_USERS_PAGE_SIZE = 20;
const AdminUsersGridSchema = {};

function positiveNumber(value: number | string | undefined, fallback: number) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function userDisabled(user: AdminUser) {
  const extended = user as AdminUser & {
    disabled?: boolean;
    is_disabled?: boolean;
    active?: boolean;
  };
  return !!(
    extended.disabled ??
    extended.is_disabled ??
    extended.active === false
  );
}

export default function AdminUsersPageContent() {
  const columns = useMemo<Column<AdminUser, AdminUsersGridParams>[]>(
    () => [
      {
        key: "id",
        header: "ID",
        className: "whitespace-nowrap",
        render: (user) => user.id,
      },
      {
        key: "email",
        header: "Email",
        className: "max-w-xs truncate",
        render: (user) => <span title={user.email}>{user.email}</span>,
      },
      {
        key: "name",
        header: "Name",
        className: "max-w-xs truncate",
        render: (user) => <span title={user.name}>{user.name || "—"}</span>,
      },
      {
        key: "verified",
        header: "Verified",
        render: (user) => (
          <StatusBadge
            status={user.email_verified ? "Verified" : "Unverified"}
          />
        ),
      },
      {
        key: "status",
        header: "Status",
        render: (user) => (
          <StatusBadge status={userDisabled(user) ? "Disabled" : "Active"} />
        ),
      },
      {
        key: "created_at",
        header: "Created",
        className: "whitespace-nowrap",
        render: (user) => formatActivityTimestamp(user.created_at),
      },
      {
        key: "actions",
        header: "",
        className: "w-12 whitespace-nowrap",
        render: (user) => <UserTaskMenu user={user} />,
      },
    ],
    [],
  );

  const useAdminUsersGridQuery = (params: AdminUsersGridParams) => {
    const page = positiveNumber(params.page, 1);
    const perPage = positiveNumber(params.per_page, ADMIN_USERS_PAGE_SIZE);
    const query = useAdminUsers({
      page,
      perPage,
      q: params.q,
      disabled:
        params.disabled === undefined ? undefined : params.disabled === "true",
    });

    return {
      data: query.data,
      isLoading: query.isLoading,
      raw: {
        metadata: {
          page: query.metadata?.page ?? page,
          per_page: query.metadata?.per_page ?? perPage,
          total: query.metadata?.total ?? 0,
        },
      },
    };
  };

  return (
    <GenericList
      title="Users"
      paramsSchema={AdminUsersGridSchema}
      useQuery={useAdminUsersGridQuery}
      columns={columns}
      renderFilters={(params, setFilter) => (
        <>
          <input
            type="search"
            aria-label="Search users by name or email"
            placeholder="Search name or email"
            className="input input-sm input-bordered w-56"
            value={params.q ?? ""}
            onChange={(event) => setFilter("q", event.target.value)}
          />
          <select
            aria-label="Filter users by account status"
            className="select select-sm select-bordered w-40"
            value={params.disabled ?? ""}
            onChange={(event) => setFilter("disabled", event.target.value)}
          >
            <option value="">All Statuses</option>
            <option value="false">Active</option>
            <option value="true">Disabled</option>
          </select>
        </>
      )}
      emptyMessage="No users found matching criteria."
    />
  );
}
