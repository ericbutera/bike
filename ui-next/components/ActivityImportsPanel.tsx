"use client";

import { auth } from "@ericbutera/kaleido";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useRef, useState } from "react";
import toast from "react-hot-toast";
import {
  extractApiMessage,
  formatActivityTimestamp,
  formatDuration,
} from "../lib/activityFormatting";
import {
  useActivityArchiveImportJobs,
  useActivityImports,
  useActivityProcessingState,
  useImportActivityArchiveUrl,
  useUploadActivityImport,
  type ActivityArchiveImportJob,
  type ActivityImport,
} from "../lib/queries";
import AuthRequiredCard from "./AuthRequiredCard";

const ALLOWED_EXTENSIONS = new Set(["fit", "tcx", "gpx"]);

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function getExtension(filename: string) {
  const parts = filename.toLowerCase().split(".");
  return parts.length > 1 ? (parts.at(-1) ?? "") : "";
}

export default function ActivityImportsPanel() {
  const authApi = auth.useAuthApi();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const router = useRouter();
  const archiveJobsQuery = useActivityArchiveImportJobs({
    enabled: !!user,
    refetchIntervalMs: 5000,
  });
  const processingStateQuery = useActivityProcessingState({
    enabled: !!user,
    refetchIntervalMs: user ? 5000 : false,
  });
  const importsQuery = useActivityImports({
    enabled: !!user,
    refetchIntervalMs: archiveJobsQuery.data.some(
      (job) => !isArchiveJobTerminal(job.status),
    )
      ? 5000
      : false,
  });
  const uploadMutation = useUploadActivityImport();
  const archiveImportMutation = useImportActivityArchiveUrl();
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
  const [archiveUrl, setArchiveUrl] = useState("");
  const isProcessingLocked = processingStateQuery.data.is_active;
  const processingMessage =
    processingStateQuery.data.message ??
    "Another activity processing job is already running. Wait for it to finish before uploading more rides.";

  const onUpload = async () => {
    if (isProcessingLocked) {
      toast.error(processingMessage);
      return;
    }

    if (selectedFiles.length === 0) {
      toast.error("Choose one or more .fit, .tcx, or .gpx files first.");
      return;
    }

    const invalidFile = selectedFiles.find(
      (file) => !ALLOWED_EXTENSIONS.has(getExtension(file.name)),
    );
    if (invalidFile) {
      toast.error(
        `Only .fit, .tcx, and .gpx files are supported. Rejected ${invalidFile.name}.`,
      );
      return;
    }

    try {
      const results: ActivityImport[] = [];

      for (const file of selectedFiles) {
        const result = await uploadMutation.uploadAsync(file);
        results.push(result);
      }

      setSelectedFiles([]);
      if (inputRef.current) {
        inputRef.current.value = "";
      }

      toast.success(buildUploadSuccessMessage(results));
    } catch (error) {
      toast.error(extractApiMessage(error));
    }
  };

  const onImportArchive = async () => {
    if (isProcessingLocked) {
      toast.error(processingMessage);
      return;
    }

    if (!archiveUrl.trim()) {
      toast.error("Paste a Garmin or Strava export URL first.");
      return;
    }

    try {
      await archiveImportMutation.importAsync(archiveUrl.trim());
      setArchiveUrl("");
      toast.success(
        "Queued archive import. Bike will fetch and process it in the background.",
      );
    } catch (error) {
      toast.error(extractApiMessage(error));
    }
  };

  if (isLoadingUser) {
    return (
      <section className="card bg-base-100 shadow-xl">
        <div className="card-body items-center py-10">
          <span className="loading loading-spinner loading-md" />
        </div>
      </section>
    );
  }

  if (!user) {
    return (
      <AuthRequiredCard
        title="Activity Imports"
        description="Sign in to upload raw activity files or fetch a Garmin or Strava export ZIP by URL. GPX is still the fastest way to test the single-file pipeline, and FIT and TCX are accepted too."
        showSignup
      />
    );
  }

  return (
    <section id="manual-upload" className="grid gap-6">
      <div className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="flex items-start justify-between gap-4">
            <div>
              <p className="text-sm text-base-content/60">Base System</p>
              <h2 className="card-title text-3xl">Upload raw activity files</h2>
            </div>
          </div>

          <fieldset className="fieldset rounded-box border border-base-300 bg-base-200 p-4">
            <legend className="fieldset-legend">Activity file</legend>
            {isProcessingLocked ? (
              <div className="alert alert-warning mb-4 text-sm">
                <span>
                  {processingMessage} Uploads will re-enable automatically when
                  the current job finishes.
                </span>
              </div>
            ) : null}
            <input
              ref={inputRef}
              type="file"
              accept=".fit,.tcx,.gpx"
              multiple
              aria-label="Activity file"
              className="file-input file-input-bordered w-full"
              onChange={(event) => {
                const files = Array.from(event.target.files ?? []);
                setSelectedFiles(files);
              }}
            />

            <div className="card bg-base-100 shadow-sm">
              <div className="card-body p-4 text-sm text-base-content/70">
                {selectedFiles.length > 0 ? (
                  <div className="space-y-3">
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <div className="font-medium text-base-content">
                        {selectedFiles.length === 1
                          ? selectedFiles[0].name
                          : `${selectedFiles.length} files selected`}
                      </div>
                      <span className="badge badge-neutral badge-outline uppercase">
                        {selectedFiles.length === 1
                          ? getExtension(selectedFiles[0].name) || "unknown"
                          : "batch"}
                      </span>
                    </div>

                    {selectedFiles.length === 1 ? (
                      <div>{formatBytes(selectedFiles[0].size)}</div>
                    ) : (
                      <div className="space-y-2">
                        {selectedFiles.map((file) => (
                          <div
                            key={`${file.name}-${file.size}`}
                            className="card bg-base-200 shadow-sm"
                          >
                            <div className="card-body flex-row items-center justify-between gap-3 p-3">
                              <div>
                                <div className="font-medium text-base-content">
                                  {file.name}
                                </div>
                                <div className="text-sm text-base-content/70">
                                  {formatBytes(file.size)}
                                </div>
                              </div>
                              <span className="badge badge-ghost uppercase">
                                {getExtension(file.name) || "unknown"}
                              </span>
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                ) : (
                  <span>
                    Choose one or more `.fit`, `.tcx`, or `.gpx` files to seed
                    the system.
                  </span>
                )}
              </div>
            </div>

            <div className="mt-4 flex flex-wrap items-center gap-3">
              <button
                type="button"
                className="btn btn-primary"
                disabled={
                  selectedFiles.length === 0 ||
                  uploadMutation.isPending ||
                  isProcessingLocked
                }
                onClick={onUpload}
              >
                {uploadMutation.isPending
                  ? selectedFiles.length > 1
                    ? "Uploading activities..."
                    : "Uploading..."
                  : selectedFiles.length > 1
                    ? "Upload activities"
                    : "Upload activity"}
              </button>
              <div className="badge badge-ghost">
                Maximum size is controlled by the API. Current default is 25 MB.
              </div>
            </div>
          </fieldset>
        </div>
      </div>

      <div className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="flex items-start justify-between gap-4">
            <div>
              <p className="text-sm text-base-content/60">Provider exports</p>
              <h2 className="card-title text-3xl">
                Fetch an export ZIP by URL
              </h2>
              <p className="mt-2 max-w-3xl text-sm text-base-content/70">
                Paste a shareable Garmin Connect or Strava export URL and Bike
                will fetch the archive server-side, unpack supported activity
                files, and deduplicate anything already in your feed.
              </p>
            </div>
          </div>

          <fieldset className="fieldset rounded-box border border-base-300 bg-base-200 p-4">
            <legend className="fieldset-legend">Archive URL</legend>
            <input
              type="url"
              aria-label="Archive URL"
              className="input input-bordered w-full"
              placeholder="https://.../export.zip"
              value={archiveUrl}
              onChange={(event) => {
                setArchiveUrl(event.target.value);
              }}
            />

            <div className="mt-4 flex flex-wrap items-center gap-3">
              <button
                type="button"
                className="btn btn-primary"
                disabled={
                  !archiveUrl.trim() ||
                  archiveImportMutation.isPending ||
                  isProcessingLocked
                }
                onClick={onImportArchive}
              >
                {archiveImportMutation.isPending
                  ? "Queueing import..."
                  : "Queue archive import"}
              </button>
              <div className="badge badge-ghost">
                Bike queues a worker task, so large exports do not need to
                finish inside the request timeout window.
              </div>
            </div>
          </fieldset>

          <div className="mt-6 rounded-2xl border border-base-300 bg-base-100 p-5 shadow-sm">
            <div className="flex items-center justify-between gap-3">
              <div>
                <div className="text-sm text-base-content/60">
                  Recent archive imports
                </div>
                <div className="mt-1 text-base font-semibold">
                  Worker-backed status updates
                </div>
              </div>
              {archiveJobsQuery.isFetching ? (
                <span className="loading loading-spinner loading-xs" />
              ) : null}
            </div>

            {archiveJobsQuery.isError ? (
              <div className="alert alert-error mt-4">
                {extractApiMessage(archiveJobsQuery.error)}
              </div>
            ) : null}

            {!archiveJobsQuery.isError && archiveJobsQuery.data.length === 0 ? (
              <div className="mt-4 rounded-xl border border-dashed border-base-300 bg-base-200 px-4 py-3 text-sm text-base-content/70">
                No archive imports queued yet.
              </div>
            ) : null}

            {archiveJobsQuery.data.length > 0 ? (
              <div className="mt-4 space-y-3">
                {archiveJobsQuery.data.map((job) => (
                  <div
                    key={job.id}
                    className="rounded-2xl border border-base-300 bg-base-200 p-4"
                  >
                    <div className="flex flex-wrap items-start justify-between gap-3">
                      <div className="min-w-0 flex-1">
                        <div className="text-sm text-base-content/60">
                          Queued {formatActivityTimestamp(job.created_at)}
                        </div>
                        <div className="mt-1 break-all font-medium text-base-content">
                          {job.resolved_url ?? job.archive_url}
                        </div>
                      </div>
                      <span className={archiveJobStatusBadgeClass(job.status)}>
                        {job.status}
                      </span>
                    </div>

                    <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2 xl:grid-cols-5">
                      <ArchiveSummaryItem
                        label="Entries"
                        value={job.total_entries}
                      />
                      <ArchiveSummaryItem
                        label="Supported"
                        value={job.supported_entry_count}
                      />
                      <ArchiveSummaryItem
                        label="Imported"
                        value={job.imported_count}
                      />
                      <ArchiveSummaryItem
                        label="Duplicates"
                        value={job.duplicate_count}
                      />
                      <ArchiveSummaryItem
                        label="Failed"
                        value={job.failed_count}
                      />
                    </dl>

                    {job.failure_message ? (
                      <div className="alert alert-error mt-4 text-sm">
                        <span>{job.failure_message}</span>
                      </div>
                    ) : null}

                    {job.error_samples && job.error_samples.length > 0 ? (
                      <div className="mt-4 space-y-2 text-sm text-base-content/80">
                        {job.error_samples.map((sample) => (
                          <div
                            key={`${job.id}-${sample}`}
                            className="rounded-xl border border-base-300 bg-base-100 px-3 py-2"
                          >
                            {sample}
                          </div>
                        ))}
                      </div>
                    ) : null}
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        </div>
      </div>

      <div className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-sm text-base-content/60">Recent imports</p>
              <h3 className="card-title text-xl">Your latest uploads</h3>
            </div>
            {importsQuery.isFetching ? (
              <span className="loading loading-spinner loading-xs" />
            ) : null}
          </div>

          {importsQuery.isError ? (
            <div className="alert alert-error">
              {extractApiMessage(importsQuery.error)}
            </div>
          ) : null}

          {!importsQuery.isError && importsQuery.data.length === 0 ? (
            <div className="alert">
              <span>
                No uploads yet. Start with one of your GPX exports to seed the
                activity stream and verify the raw ingest path.
              </span>
            </div>
          ) : null}

          {importsQuery.data.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="table table-zebra table-sm">
                <thead>
                  <tr>
                    <th>Date</th>
                    <th>Location</th>
                    <th className="text-right">Duration</th>
                  </tr>
                </thead>
                <tbody>
                  {importsQuery.data.map((activityImport) => {
                    const startedAt =
                      activityImport.activity_started_at ??
                      activityImport.created_at;
                    const href = activityImport.activity_id
                      ? `/activities/${activityImport.activity_id}`
                      : null;

                    return (
                      <tr
                        key={activityImport.id}
                        className={
                          href
                            ? "cursor-pointer transition hover:bg-base-100"
                            : undefined
                        }
                        title={activityImport.original_filename}
                        onClick={() => {
                          if (href) {
                            router.push(href);
                          }
                        }}
                      >
                        <td className="whitespace-nowrap font-medium text-base-content">
                          {href ? (
                            <Link
                              href={href}
                              className="transition hover:text-primary"
                            >
                              {formatActivityTimestamp(startedAt)}
                            </Link>
                          ) : (
                            formatActivityTimestamp(startedAt)
                          )}
                        </td>
                        <td className="text-base-content/65">
                          {activityImport.activity_location ?? "--"}
                        </td>
                        <td className="text-right font-medium text-base-content">
                          {formatDuration(
                            activityImport.activity_duration_seconds,
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          ) : null}
        </div>
      </div>
    </section>
  );
}

function buildUploadSuccessMessage(results: ActivityImport[]) {
  const importedCount = results.filter(
    (result) => result.status !== "duplicate",
  ).length;
  const duplicateCount = results.filter(
    (result) => result.status === "duplicate",
  ).length;

  if (results.length === 1) {
    const [result] = results;

    return result.status === "duplicate"
      ? `Already had ${result.original_filename} in your activity feed.`
      : `Imported ${result.original_filename} into your activity feed.`;
  }

  if (importedCount > 0 && duplicateCount > 0) {
    return `Imported ${importedCount} activities and skipped ${duplicateCount} duplicates.`;
  }

  if (duplicateCount > 0) {
    return `Skipped ${duplicateCount} duplicate activities.`;
  }

  return `Imported ${importedCount} activities into your activity feed.`;
}

function ArchiveSummaryItem({
  label,
  value,
}: {
  label: string;
  value: number;
}) {
  return (
    <div className="rounded-xl bg-base-200 px-4 py-3">
      <dt className="text-base-content/60">{label}</dt>
      <dd className="mt-1 text-lg font-semibold">{value}</dd>
    </div>
  );
}

function isArchiveJobTerminal(status: string) {
  return status === "succeeded" || status === "failed";
}

function archiveJobStatusBadgeClass(
  status: ActivityArchiveImportJob["status"],
) {
  switch (status) {
    case "queued":
      return "badge badge-warning badge-outline uppercase";
    case "running":
      return "badge badge-info badge-outline uppercase";
    case "succeeded":
      return "badge badge-success badge-outline uppercase";
    case "failed":
      return "badge badge-error badge-outline uppercase";
    default:
      return "badge badge-ghost uppercase";
  }
}
