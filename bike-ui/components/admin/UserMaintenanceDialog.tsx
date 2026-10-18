"use client";

import Link from "next/link";
import { useState, type FormEvent } from "react";
import {
  useAdminUserArchiveImport,
  useCleanupUserDuplicateActivities,
  type AdminUser,
} from "../../lib/queries";
import { handleApiError } from "../../lib/api";
import Modal from "../ui/Modal";

export type UserMaintenanceAction = "archive" | "duplicates";

export default function UserMaintenanceDialog({
  user,
  action,
  onClose,
}: {
  user: AdminUser;
  action: UserMaintenanceAction;
  onClose: () => void;
}) {
  const archive = useAdminUserArchiveImport(user.id);
  const cleanup = useCleanupUserDuplicateActivities();
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string>();
  const [result, setResult] = useState<{
    message: string;
    archiveJobId?: number;
  }>();
  const pending = archive.isPending || cleanup.isPending;
  const isArchive = action === "archive";

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError(undefined);
    try {
      if (isArchive) {
        const job = await archive.importAsync(url.trim());
        setResult({
          message:
            "Archive import queued for this user. It will continue in the background.",
          archiveJobId: job.id,
        });
      } else {
        const summary = await cleanup.cleanupAsync(user.id);
        setResult({
          message: `${summary.message} Deleted: ${summary.deleted_activity_count}. Retained: ${summary.retained_activity_count}.`,
        });
      }
    } catch (failure) {
      setError(handleApiError(failure).message ?? "Request failed. Try again.");
    }
  }

  return (
    <Modal
      title={
        isArchive
          ? "Fetch provider export archives"
          : "Clean up duplicate activities"
      }
      onClose={() => {
        if (!pending) onClose();
      }}
    >
      <div className="mt-4 space-y-4">
        <p className="font-medium">
          {user.name || user.email} (#{user.id})
        </p>
        <p className="text-sm text-base-content/75">
          {isArchive
            ? "Fetch a Garmin Connect or Strava export URL and import its supported activity files into this user's account."
            : "Keep the preferred copy of each duplicate activity for this user, remove extra copies, and refresh affected results."}
        </p>
        {result ? (
          <div role="status" className="space-y-3">
            <p>{result.message}</p>
            {result.archiveJobId ? (
              <Link
                className="link"
                href={`/admin/imports?archive_job_id=${result.archiveJobId}`}
              >
                View archive imports
              </Link>
            ) : null}
          </div>
        ) : (
          <form className="space-y-4" onSubmit={submit}>
            {isArchive ? (
              <label className="form-control">
                <span>Archive URL</span>
                <input
                  type="url"
                  required
                  className="input input-bordered w-full"
                  placeholder="https://…/export.zip"
                  value={url}
                  disabled={pending}
                  onChange={(event) => setUrl(event.target.value)}
                />
              </label>
            ) : null}
            {error ? (
              <p role="alert" className="text-error">
                {error}
              </p>
            ) : null}
            <button
              type="submit"
              className="btn btn-primary"
              disabled={pending}
            >
              {pending
                ? "Submitting…"
                : isArchive
                  ? "Queue archive import"
                  : "Clean duplicates"}
            </button>
          </form>
        )}
      </div>
    </Modal>
  );
}
