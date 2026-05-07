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
import { useActivityImports, useUploadActivityImport } from "../lib/queries";
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
  const importsQuery = useActivityImports({ enabled: !!user });
  const uploadMutation = useUploadActivityImport();
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);

  const onUpload = async () => {
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
      const results = [] as Array<{ original_filename: string }>;

      for (const file of selectedFiles) {
        const result = await uploadMutation.uploadAsync(file);
        results.push({ original_filename: result.original_filename });
      }

      setSelectedFiles([]);
      if (inputRef.current) {
        inputRef.current.value = "";
      }

      if (results.length === 1) {
        toast.success(
          `Imported ${results[0].original_filename} into your activity feed.`,
        );
      } else {
        toast.success(
          `Imported ${results.length} activities into your activity feed.`,
        );
      }
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
        title="Manual Activity Uploads"
        description="Sign in to upload a raw activity file. GPX is the fastest way to test the pipeline right now, and FIT and TCX are accepted too."
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
                  selectedFiles.length === 0 || uploadMutation.isPending
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
