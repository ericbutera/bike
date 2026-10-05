"use client";

import { useMemo, useState, type FormEvent } from "react";
import toast from "react-hot-toast";
import GenericList, { type Column } from "../ui/GenericList";
import {
  useAdminUser,
  useAdminUsers,
  useDisableAdminUser,
  useUpdateAdminUser,
  type AdminUser,
} from "../../lib/queries";
import { useKeyedState } from "../../lib/useKeyedState";

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
  const [selectedUser, setSelectedUser] = useState<AdminUser | null>(null);
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
          <span
            className={`badge ${user.email_verified ? "badge-success" : "badge-warning"}`}
          >
            {user.email_verified ? "Verified" : "Unverified"}
          </span>
        ),
      },
      {
        key: "status",
        header: "Status",
        render: (user) => (
          <span
            className={`badge ${userDisabled(user) ? "badge-error" : "badge-success"}`}
          >
            {userDisabled(user) ? "Disabled" : "Active"}
          </span>
        ),
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
    <>
      <GenericList
        title="Users"
        paramsSchema={AdminUsersGridSchema}
        useQuery={useAdminUsersGridQuery}
        columns={columns}
        onRowClick={setSelectedUser}
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
      <AdminUserModal
        user={selectedUser}
        onClose={() => setSelectedUser(null)}
      />
    </>
  );
}

function AdminUserModal({
  user,
  onClose,
}: {
  user: AdminUser | null;
  onClose: () => void;
}) {
  const detailQuery = useAdminUser(user?.id ?? null);
  const updateUser = useUpdateAdminUser();
  const disableUser = useDisableAdminUser();
  const detailUser = detailQuery.data ?? user;
  const draftKey = [
    user?.id ?? "none",
    detailUser?.name ?? "",
    detailUser?.is_admin ?? false,
    detailUser ? userDisabled(detailUser) : false,
  ].join(":");
  const [draft, setDraft] = useKeyedState(draftKey, {
    name: detailUser?.name ?? "",
    isAdmin: detailUser?.is_admin ?? false,
    accountDisabled: detailUser ? userDisabled(detailUser) : false,
    saveError: null as string | null,
  });
  const { name, isAdmin, accountDisabled, saveError } = draft;
  const setName = (value: string) =>
    setDraft((current) => ({ ...current, name: value }));
  const setIsAdmin = (value: boolean) =>
    setDraft((current) => ({ ...current, isAdmin: value }));
  const setAccountDisabled = (value: boolean) =>
    setDraft((current) => ({ ...current, accountDisabled: value }));
  const setSaveError = (value: string | null) =>
    setDraft((current) => ({ ...current, saveError: value }));
  const isBusy = updateUser.isPending || disableUser.isPending;

  if (!user) return null;
  const selectedUser = user;

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSaveError(null);
    try {
      await updateUser.updateAsync(selectedUser.id, {
        name: name || undefined,
        is_admin: isAdmin,
      });
      toast.success("User updated");
      onClose();
    } catch (error) {
      console.error("Failed to save user", error);
      setSaveError("Failed to save user.");
      toast.error("Failed to save user");
    }
  }

  async function disableAccount() {
    try {
      await disableUser.disableAsync(selectedUser.id, true);
      setAccountDisabled(true);
      toast.success("User account disabled");
    } catch (error) {
      console.error("Failed to disable user account", error);
      toast.error("Failed to disable user account");
    }
  }

  return (
    <div
      className="modal modal-open"
      role="dialog"
      aria-modal="true"
      aria-labelledby="admin-user-title"
      onClick={onClose}
    >
      <div
        className="modal-box max-w-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 id="admin-user-title" className="mb-4 text-lg font-bold">
          Edit User #{user.id}
        </h2>

        {detailQuery.isLoading ? (
          <p className="mb-4 text-sm text-base-content/65">Loading user…</p>
        ) : null}
        {detailQuery.error ? (
          <p role="alert" className="mb-4 text-sm text-error">
            Unable to load user detail.
          </p>
        ) : null}

        <form onSubmit={save} className="space-y-4">
          <label className="form-control w-full">
            <span className="label-text mb-1">Email</span>
            <input
              type="email"
              className="input input-bordered w-full"
              value={detailUser?.email ?? user.email}
              readOnly
              disabled={isBusy}
            />
          </label>

          <label className="form-control w-full">
            <span className="label-text mb-1">Name</span>
            <input
              type="text"
              className="input input-bordered w-full"
              value={name}
              onChange={(event) => setName(event.target.value)}
              disabled={isBusy}
            />
          </label>

          <label className="label cursor-pointer justify-start gap-2">
            <input
              type="checkbox"
              className="checkbox checkbox-sm"
              checked={isAdmin}
              onChange={(event) => setIsAdmin(event.target.checked)}
              disabled={isBusy}
            />
            <span>Admin user</span>
          </label>

          <div className="space-y-2 rounded-lg border border-base-300 p-3">
            <div className="text-sm font-semibold">Account Status</div>
            <button
              type="button"
              className="btn btn-sm btn-outline btn-error"
              onClick={() => void disableAccount()}
              disabled={isBusy || accountDisabled}
            >
              {accountDisabled ? "Account Disabled" : "Disable Account"}
            </button>
          </div>

          {saveError ? (
            <p role="alert" className="text-sm text-error">
              {saveError}
            </p>
          ) : null}

          <div className="modal-action">
            <button
              type="button"
              className="btn btn-ghost"
              onClick={onClose}
              disabled={isBusy}
            >
              Close
            </button>
            <button type="submit" className="btn btn-primary" disabled={isBusy}>
              Save
            </button>
          </div>
        </form>
      </div>
      <button
        type="button"
        className="modal-backdrop cursor-default"
        aria-label="Close user details"
      />
    </div>
  );
}
