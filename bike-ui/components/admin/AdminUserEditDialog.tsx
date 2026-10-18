"use client";

import { useState, type FormEvent } from "react";
import toast from "react-hot-toast";
import {
  useDisableAdminUser,
  useUpdateAdminUser,
  type AdminUser,
} from "../../lib/queries";
import Modal from "../ui/Modal";

export default function AdminUserEditDialog({
  user,
  onClose,
}: {
  user: AdminUser;
  onClose: () => void;
}) {
  const updateUser = useUpdateAdminUser();
  const disableUser = useDisableAdminUser();
  const [name, setName] = useState(user.name);
  const [isAdmin, setIsAdmin] = useState(user.is_admin);
  const [disabled, setDisabled] = useState(user.disabled);
  const [error, setError] = useState<string>();
  const busy = updateUser.isPending || disableUser.isPending;

  async function save(event: FormEvent) {
    event.preventDefault();
    setError(undefined);
    try {
      await updateUser.updateAsync(user.id, {
        name: name || undefined,
        is_admin: isAdmin,
      });
      toast.success("User updated");
      onClose();
    } catch {
      setError("Failed to save user.");
    }
  }

  async function disableAccount() {
    setError(undefined);
    try {
      await disableUser.disableAsync(user.id, true);
      setDisabled(true);
      toast.success("User account disabled");
    } catch {
      setError("Failed to disable user account.");
    }
  }

  return (
    <Modal title={`Edit User #${user.id}`} onClose={onClose}>
      <form onSubmit={save} className="mt-4 space-y-4">
        <label className="form-control block">
          <span className="label-text mb-1 block">Email</span>
          <input
            type="email"
            className="input input-bordered w-full"
            value={user.email}
            readOnly
          />
        </label>
        <label className="form-control block">
          <span className="label-text mb-1 block">Name</span>
          <input
            className="input input-bordered w-full"
            value={name}
            onChange={(event) => setName(event.target.value)}
            disabled={busy}
          />
        </label>
        <label className="label cursor-pointer justify-start gap-2">
          <input
            type="checkbox"
            className="checkbox checkbox-sm"
            checked={isAdmin}
            onChange={(event) => setIsAdmin(event.target.checked)}
            disabled={busy}
          />
          <span>Admin user</span>
        </label>
        <div className="space-y-2 rounded-lg border border-base-300 p-3">
          <div className="text-sm font-semibold">Account Status</div>
          <button
            type="button"
            className="btn btn-sm btn-outline btn-error"
            onClick={() => void disableAccount()}
            disabled={busy || disabled}
          >
            {disabled ? "Account Disabled" : "Disable Account"}
          </button>
        </div>
        {error ? (
          <p role="alert" className="text-error">
            {error}
          </p>
        ) : null}
        <button type="submit" className="btn btn-primary" disabled={busy}>
          Save
        </button>
      </form>
    </Modal>
  );
}
