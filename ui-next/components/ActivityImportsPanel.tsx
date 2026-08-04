"use client";

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
  useImportActivityArchiveUrl,
  useUploadActivityImport,
  type ActivityArchiveImportJob,
  type ActivityImport,
} from "../lib/queries";

const ALLOWED_EXTENSIONS = new Set(["fit", "tcx", "gpx"]);
const MANUAL_IMPORT_VISIBLE_ROW_COUNT = 10;
const MANUAL_IMPORT_QUEUE_MAX_HEIGHT = "34rem";

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
  const router = useRouter();
  const archiveJobsQuery = useActivityArchiveImportJobs({
    refetchIntervalMs: 5000,
  });
  const importsQuery = useActivityImports({
    refetchIntervalMs: 5000,
  });
  const uploadMutation = useUploadActivityImport();
  const archiveImportMutation = useImportActivityArchiveUrl();
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
  const [archiveUrl, setArchiveUrl] = useState("");
  const [isUploadingFiles, setIsUploadingFiles] = useState(false);
  const activeUploadCount = importsQuery.data.filter(
    isActivityImportActive,
  ).length;
  const shouldLimitManualImportQueue =
    importsQuery.data.length > MANUAL_IMPORT_VISIBLE_ROW_COUNT;

  const onUpload = async () => {
    if (isUploadingFiles) {
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

    setIsUploadingFiles(true);

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
    } finally {
      setIsUploadingFiles(false);
    }
  };

  const onImportArchive = async () => {
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
                disabled={selectedFiles.length === 0 || isUploadingFiles}
                onClick={onUpload}
              >
                {isUploadingFiles ? (
                  <>
                    <span className="loading loading-spinner loading-xs" />
                    {selectedFiles.length > 1
                      ? "Queueing activities..."
                      : "Queueing activity..."}
                  </>
                ) : selectedFiles.length > 1 ? (
                  "Upload activities"
                ) : (
                  "Upload activity"
                )}
              </button>
              <div className="badge badge-ghost">
                Maximum size is controlled by the API. Current default is 25 MB.
              </div>
            </div>
          </fieldset>

          <section className="mt-6 rounded-box border border-base-300 bg-base-100 p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <p className="text-sm text-base-content/60">Upload queue</p>
                <h3 className="text-lg font-semibold">
                  Manual file processing
                </h3>
              </div>
              <div className="flex items-center gap-2">
                {activeUploadCount > 0 ? (
                  <span className="badge badge-info badge-outline">
                    {activeUploadCount} active
                  </span>
                ) : null}
                {importsQuery.isFetching ? (
                  <span className="loading loading-spinner loading-xs" />
                ) : null}
              </div>
            </div>

            {importsQuery.isError ? (
              <div className="alert alert-error mt-4">
                {extractApiMessage(importsQuery.error)}
              </div>
            ) : null}

            {!importsQuery.isError && importsQuery.data.length === 0 ? (
              <div className="alert mt-4">
                <span>
                  No uploads yet. Start with one of your GPX exports to seed
                  the queue and verify the raw ingest path.
                </span>
              </div>
            ) : null}

            {importsQuery.data.length > 0 ? (
              <div
                className={
                  shouldLimitManualImportQueue
                    ? "mt-4 overflow-x-auto overflow-y-auto"
                    : "mt-4 overflow-x-auto"
                }
                data-testid="manual-upload-queue-scroll"
                style={
                  shouldLimitManualImportQueue
                    ? { maxHeight: MANUAL_IMPORT_QUEUE_MAX_HEIGHT }
                    : undefined
                }
              >
                <table className="table table-zebra table-sm">
                  <thead className="sticky top-0 z-10 bg-base-100">
                    <tr>
                      <th>Status</th>
                      <th>File</th>
                      <th className="whitespace-nowrap">Queued</th>
                      <th>Result</th>
                    </tr>
                  </thead>
                  <tbody>
                    {importsQuery.data.map((activityImport) => {
                      const href = activityImport.activity_id
                        ? `/activities/${activityImport.activity_id}`
                        : null;
                      const detail = formatActivityImportDetail(activityImport);

                      return (
                        <tr
                          key={activityImport.id}
                          className={
                            href
                              ? "h-12 cursor-pointer transition hover:bg-base-100"
                              : "h-12"
                          }
                          title={activityImport.original_filename}
                          onClick={() => {
                            if (href) {
                              router.push(href);
                            }
                          }}
                        >
                          <td>
                            <span
                              className={activityImportStatusBadgeClass(
                                activityImport,
                              )}
                            >
                              {formatActivityImportStatus(activityImport)}
                            </span>
                          </td>
                          <td className="min-w-[12rem] max-w-xs">
                            <div className="truncate font-medium text-base-content">
                              {activityImport.original_filename}
                            </div>
                            <div className="text-xs uppercase text-base-content/50">
                              {activityImport.format}
                            </div>
                          </td>
                          <td className="whitespace-nowrap text-base-content/65">
                            {formatActivityTimestamp(activityImport.created_at)}
                          </td>
                          <td className="text-base-content/65">
                            {href ? (
                              <Link
                                href={href}
                                className="transition hover:text-primary"
                              >
                                {detail}
                              </Link>
                            ) : (
                              detail
                            )}
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            ) : null}
          </section>
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
                disabled={!archiveUrl.trim() || archiveImportMutation.isPending}
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

          <details className="collapse-arrow collapse mt-6 rounded-box border border-base-300 bg-base-100 shadow-sm">
            <summary className="collapse-title flex items-center justify-between gap-3">
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
            </summary>

            <div className="collapse-content">
              {archiveJobsQuery.isError ? (
                <div className="alert alert-error mt-4">
                  {extractApiMessage(archiveJobsQuery.error)}
                </div>
              ) : null}

              {!archiveJobsQuery.isError &&
              archiveJobsQuery.data.length === 0 ? (
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
                        <span
                          className={archiveJobStatusBadgeClass(job.status)}
                        >
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
          </details>
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
      : `Queued ${result.original_filename} for processing.`;
  }

  if (importedCount > 0 && duplicateCount > 0) {
    return `Queued ${importedCount} activities and skipped ${duplicateCount} duplicates.`;
  }

  if (duplicateCount > 0) {
    return `Skipped ${duplicateCount} duplicate activities.`;
  }

  return `Queued ${importedCount} activities for processing.`;
}

function isActivityImportActive(activityImport: ActivityImport) {
  return activityImport.status === "processing";
}

function formatActivityImportStatus(activityImport: ActivityImport) {
  if (
    activityImport.status === "processing" &&
    activityImport.processing_stage === "raw_stored"
  ) {
    return "queued";
  }

  if (activityImport.status === "uploaded") {
    return "processed";
  }

  return activityImport.status;
}

function activityImportStatusBadgeClass(activityImport: ActivityImport) {
  switch (formatActivityImportStatus(activityImport)) {
    case "queued":
      return "badge badge-warning badge-outline uppercase";
    case "processing":
      return "badge badge-info badge-outline uppercase";
    case "processed":
      return "badge badge-success badge-outline uppercase";
    case "duplicate":
      return "badge badge-neutral badge-outline uppercase";
    case "failed":
      return "badge badge-error badge-outline uppercase";
    default:
      return "badge badge-ghost uppercase";
  }
}

function formatActivityImportDetail(activityImport: ActivityImport) {
  if (activityImport.status === "failed") {
    return activityImport.processing_error ?? "Processing failed";
  }

  if (activityImport.status === "duplicate") {
    return "Duplicate of existing activity";
  }

  if (activityImport.status === "processing") {
    return formatActivityImportStage(activityImport.processing_stage);
  }

  const duration = formatDuration(activityImport.activity_duration_seconds);
  const parts = [
    activityImport.activity_location,
    duration === "--" ? null : duration,
  ].filter(Boolean);

  return parts.length > 0 ? parts.join(" - ") : "Open activity";
}

function formatActivityImportStage(stage: string) {
  switch (stage) {
    case "raw_stored":
      return "Waiting for worker";
    case "activity_saved":
      return "Activity saved";
    case "segments_built":
      return "Segments built";
    case "segment_analytics_built":
      return "Segment analytics built";
    case "activity_analytics_built":
      return "Activity analytics built";
    case "training_analysis_built":
      return "Training analysis built";
    case "complete":
      return "Complete";
    default:
      return "Processing";
  }
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
