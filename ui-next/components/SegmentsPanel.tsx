"use client";

import { auth } from "@ericbutera/kaleido";
import { faCrown } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import type { ReactNode } from "react";
import { useRef, useState } from "react";
import toast from "react-hot-toast";
import {
  extractApiMessage,
  formatDistance,
  formatDuration,
} from "../lib/activityFormatting";
import { useSegments, useUploadSegment } from "../lib/queries";
import { useUnitPreferences } from "../lib/unitPreferences";
import AuthRequiredCard from "./AuthRequiredCard";

const ALLOWED_EXTENSIONS = new Set(["gpx", "tcx"]);

function getExtension(filename: string) {
  const parts = filename.toLowerCase().split(".");
  return parts.length > 1 ? (parts.at(-1) ?? "") : "";
}

function SegmentMetric({ label, value }: { label: ReactNode; value: string }) {
  return (
    <div className="stats bg-base-100 shadow sm:min-w-[8.5rem]">
      <div className="stat px-3 py-2">
        <div className="stat-title">{label}</div>
        <div className="stat-value text-base">{value}</div>
      </div>
    </div>
  );
}

export default function SegmentsPanel() {
  const authApi = auth.useAuthApi();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const { unitSystem } = useUnitPreferences();
  const segmentsQuery = useSegments({ enabled: !!user });
  const uploadMutation = useUploadSegment();
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);

  const onUpload = async () => {
    if (!selectedFile) {
      toast.error("Choose a .gpx or .tcx segment export first.");
      return;
    }

    const extension = getExtension(selectedFile.name);
    if (!ALLOWED_EXTENSIONS.has(extension)) {
      toast.error("Segments currently require .gpx or .tcx route exports.");
      return;
    }

    try {
      const result = await uploadMutation.uploadAsync(selectedFile);
      setSelectedFile(null);
      if (inputRef.current) {
        inputRef.current.value = "";
      }
      toast.success(
        `Imported ${result.title} and matched ${result.effort_count} effort${
          result.effort_count === 1 ? "" : "s"
        }.`,
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
        title="Manual Segment Imports"
        description="Sign in to upload a GPX or TCX export for a segment. Bike stores the route definition and matches it against your uploaded activities so you can compare repeated efforts without setting up Strava API keys."
        showSignup
      />
    );
  }

  return (
    <section className="grid gap-6">
      <div className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="flex items-start justify-between gap-4">
            <div>
              <p className="text-sm text-base-content/60">Segments</p>
              <h2 className="card-title text-3xl">Build or import segments</h2>
              <p className="mt-2 max-w-2xl text-sm text-base-content/70">
                Crop a segment directly from one of your rides, or import a GPX
                or TCX route file when you already have the trace.
              </p>
            </div>
          </div>

          <fieldset className="fieldset rounded-box border border-base-300 bg-base-200 p-4">
            <legend className="fieldset-legend">Segment route file</legend>
            <input
              ref={inputRef}
              type="file"
              accept=".gpx,.tcx"
              aria-label="Segment route file"
              className="file-input file-input-bordered w-full"
              onChange={(event) => {
                const file = event.target.files?.[0] ?? null;
                setSelectedFile(file);
              }}
            />

            <div className="card bg-base-100 shadow-sm">
              <div className="card-body p-4 text-sm text-base-content/70">
                {selectedFile ? (
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <div className="font-medium text-base-content">
                        {selectedFile.name}
                      </div>
                      <div className="text-sm text-base-content/70">
                        Route geometry is required, so FIT is not supported here
                        yet.
                      </div>
                    </div>
                    <span className="badge badge-neutral badge-outline uppercase">
                      {getExtension(selectedFile.name) || "unknown"}
                    </span>
                  </div>
                ) : (
                  <span>
                    Choose one `.gpx` or `.tcx` file that traces the segment.
                  </span>
                )}
              </div>
            </div>

            <div className="mt-4 flex flex-wrap items-center gap-3">
              <button
                type="button"
                className="btn btn-primary"
                disabled={!selectedFile || uploadMutation.isPending}
                onClick={onUpload}
              >
                {uploadMutation.isPending ? "Importing..." : "Import segment"}
              </button>
              <div className="badge badge-ghost">
                GPX and TCX work best because they keep route coordinates
                explicit.
              </div>
            </div>
          </fieldset>
        </div>
      </div>

      <div className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-sm text-base-content/60">Recent segments</p>
              <h3 className="card-title text-xl">Comparison-ready routes</h3>
            </div>
            {segmentsQuery.isFetching ? (
              <span className="loading loading-spinner loading-xs" />
            ) : null}
          </div>

          {segmentsQuery.isError ? (
            <div className="alert alert-error">
              {extractApiMessage(segmentsQuery.error)}
            </div>
          ) : null}

          {!segmentsQuery.isError && segmentsQuery.data.length === 0 ? (
            <div className="alert">
              <span>
                No imported segments yet. Start with one GPX or TCX segment
                export, then Bike will match attempts from your existing rides.
              </span>
            </div>
          ) : null}

          <div className="space-y-3">
            {segmentsQuery.data.map((segment) => (
              <article key={segment.id} className="card bg-base-200 shadow-sm">
                <div className="card-body p-4">
                  <div className="flex items-start justify-between gap-3">
                    <div>
                      <Link
                        href={`/segments/${segment.id}`}
                        className="font-medium text-base-content transition hover:text-primary"
                      >
                        {segment.title}
                      </Link>
                    </div>
                    {segment.format ? (
                      <span className="badge badge-outline uppercase">
                        {segment.format}
                      </span>
                    ) : null}
                  </div>

                  <div className="grid gap-2 sm:grid-cols-4">
                    <SegmentMetric
                      label="Efforts"
                      value={`${segment.effort_count}`}
                    />
                    <SegmentMetric
                      label="Distance"
                      value={formatDistance(
                        segment.distance_meters,
                        unitSystem,
                      )}
                    />
                    <SegmentMetric
                      label={
                        <span className="inline-flex items-center gap-1">
                          <FontAwesomeIcon
                            icon={faCrown}
                            className="h-3 w-3 text-warning"
                          />
                          <span>KOM</span>
                        </span>
                      }
                      value={formatDuration(
                        segment.best_duration_seconds ?? null,
                      )}
                    />
                    <SegmentMetric
                      label="Your PR"
                      value={formatDuration(
                        segment.current_user_pr_duration_seconds ?? null,
                      )}
                    />
                  </div>
                </div>
              </article>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
