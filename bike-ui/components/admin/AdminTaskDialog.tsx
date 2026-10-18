"use client";

import Link from "next/link";
import { useState, type FormEvent } from "react";
import { handleApiError } from "../../lib/api";
import {
  useAdminTask,
  type AcceptedTask,
  type TaskParameters,
  type TaskTarget,
} from "../../lib/adminTasks";
import Modal from "../ui/Modal";
import TemporalLink from "../ui/TemporalLink";
import type { TaskOption } from "./taskActions";

export default function AdminTaskDialog({
  target,
  option,
  onClose,
}: {
  target: TaskTarget;
  option: TaskOption;
  onClose: () => void;
}) {
  const mutation = useAdminTask(target);
  const [archiveOnly, setArchiveOnly] = useState(false);
  const [after, setAfter] = useState("");
  const [before, setBefore] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [accepted, setAccepted] = useState<AcceptedTask | null>(null);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    let parameters: TaskParameters;
    if (option.task === "strava_backfill") {
      if (!after || !before || before < after) {
        setError("Choose a start date and an end date on or after it.");
        return;
      }
      const end = new Date(before + "T00:00:00Z");
      end.setUTCDate(end.getUTCDate() + 1);
      parameters = {
        task: option.task,
        after: new Date(after + "T00:00:00Z").toISOString(),
        before: end.toISOString(),
      };
    } else if (option.task === "reprocess_imports") {
      parameters = { task: option.task, archive_fits_only: archiveOnly };
    } else {
      parameters = { task: option.task };
    }
    try {
      setAccepted(await mutation.submit(parameters));
    } catch (failure) {
      setError(
        handleApiError(failure).message ?? "Task submission failed. Try again.",
      );
    }
  }

  return (
    <Modal
      title={option.label}
      onClose={() => {
        if (!mutation.isPending) onClose();
      }}
    >
      <div className="mt-4 space-y-4">
        <p className="font-medium">{target.label}</p>
        <p className="text-sm text-base-content/75">{option.description}</p>
        {target.kind === "activity" && option.task === "rebuild_fitness" ? (
          <p className="text-sm">
            Applies to all activities for user #{target.userId}.
          </p>
        ) : null}
        {accepted ? (
          <div role="status" className="space-y-2">
            <p>Task accepted. It will continue in the background.</p>
            <TemporalLink
              workflowId={accepted.job.job_id}
              namespace={accepted.job.namespace}
            />
            <p>
              <Link className="link" href={historyLink(target)}>
                View integration events
              </Link>
            </p>
          </div>
        ) : (
          <form className="space-y-4" onSubmit={submit}>
            {option.fields === "imports" ? (
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  className="checkbox checkbox-sm"
                  checked={archiveOnly}
                  disabled={mutation.isPending}
                  onChange={(event) => setArchiveOnly(event.target.checked)}
                />
                Archive FIT imports only
              </label>
            ) : null}
            {option.fields === "dates" ? (
              <fieldset
                disabled={mutation.isPending}
                className="grid grid-cols-2 gap-3"
              >
                <label>
                  Start date
                  <input
                    type="date"
                    required
                    className="input input-bordered w-full"
                    value={after}
                    onChange={(event) => setAfter(event.target.value)}
                  />
                </label>
                <label>
                  End date
                  <input
                    type="date"
                    required
                    min={after}
                    className="input input-bordered w-full"
                    value={before}
                    onChange={(event) => setBefore(event.target.value)}
                  />
                </label>
                <p className="col-span-2 text-xs text-base-content/70">
                  Both dates are included, using UTC days.
                </p>
              </fieldset>
            ) : null}
            {error ? (
              <p role="alert" className="text-sm text-error">
                {error}
              </p>
            ) : null}
            <button
              type="submit"
              className="btn btn-primary"
              disabled={mutation.isPending}
            >
              {mutation.isPending ? "Submitting…" : "Run task"}
            </button>
          </form>
        )}
      </div>
    </Modal>
  );
}

function historyLink(target: TaskTarget) {
  const params = new URLSearchParams({
    provider: "",
    user_id: String(target.userId),
  });
  if (target.kind === "activity") params.set("activity_id", String(target.id));
  return `/admin/integrations?${params}`;
}
