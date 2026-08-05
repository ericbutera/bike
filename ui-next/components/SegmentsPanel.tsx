"use client";

import { GenericList, type Column } from "@ericbutera/kaleido";
import { faStar } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { useRef, useState } from "react";
import toast from "react-hot-toast";
import { MemoryRouter } from "react-router-dom";
import { formatDistance, formatDuration } from "../lib/activityFormatting";
import {
  useSegments,
  useUpdateSegment,
  useUploadSegment,
  type Segment,
} from "../lib/queries";
import { useUnitPreferences } from "../lib/unitPreferences";
import InfoTooltip from "./ui/InfoTooltip";
import { LoadingSpinner } from "./ui/QueryState";

const ALLOWED_EXTENSIONS = new Set(["gpx", "tcx"]);
const SEGMENTS_PAGE_SIZE = 25;
const SEGMENT_BUILDER_HELP_TEXT =
  "Crop a segment directly from one of your rides, or import a GPX or TCX route file when you already have the trace.";

type SegmentsGridParams = {
  q?: string;
  mode?: Segment["mode"] | "";
  page?: number | string;
  per_page?: number | string;
};

const SegmentsGridSchema = {} as never;

function getExtension(filename: string) {
  const parts = filename.toLowerCase().split(".");
  return parts.length > 1 ? (parts.at(-1) ?? "") : "";
}

function segmentModeBadgeClass(mode: Segment["mode"]) {
  return mode === "dh"
    ? "badge badge-warning badge-outline font-semibold uppercase"
    : "badge badge-info badge-outline font-semibold uppercase";
}

function filterSegments(segments: Segment[], params: SegmentsGridParams) {
  const query = params.q?.trim().toLowerCase() ?? "";

  return segments
    .filter((segment) => {
      if (params.mode && segment.mode !== params.mode) {
        return false;
      }

      if (!query) {
        return true;
      }

      const haystacks = [
        segment.title,
        segment.source,
        segment.original_filename ?? "",
        segment.format ?? "",
        segment.mode,
      ];

      return haystacks.some((value) => value.toLowerCase().includes(query));
    })
    .sort(
      (left, right) =>
        left.title.localeCompare(right.title, undefined, {
          sensitivity: "base",
        }) || left.id - right.id,
    );
}

function normalizeSegmentsGridParams(params: SegmentsGridParams) {
  const page = Number(params.page);
  const perPage = Number(params.per_page);

  return {
    ...params,
    page: Number.isFinite(page) && page > 0 ? page : 1,
    per_page:
      Number.isFinite(perPage) && perPage > 0 ? perPage : SEGMENTS_PAGE_SIZE,
  };
}

export default function SegmentsPanel() {
  const { unitSystem } = useUnitPreferences();
  const segmentsQuery = useSegments();
  const updateSegmentMutation = useUpdateSegment();
  const uploadMutation = useUploadSegment();
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);

  const segmentColumns: Column<Segment, SegmentsGridParams>[] = [
    {
      key: "title",
      header: "Segment",
      render: (segment) => (
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <button
              type="button"
              className={`btn btn-ghost btn-xs btn-square ${segment.starred ? "text-warning" : "text-base-content/45"}`}
              aria-label={`${segment.starred ? "Unstar" : "Star"} ${segment.title}`}
              aria-pressed={!!segment.starred}
              disabled={updateSegmentMutation.isPending}
              onClick={() => {
                void updateSegmentMutation.updateAsync({
                  id: segment.id,
                  starred: !segment.starred,
                });
              }}
            >
              <FontAwesomeIcon icon={faStar} className="h-3.5 w-3.5" />
            </button>
            <Link
              href={`/segments/${segment.id}`}
              className="font-medium text-base-content transition hover:text-primary"
            >
              {segment.title}
            </Link>
          </div>
          <div className="mt-1 text-xs text-base-content/60">
            Imported from {segment.source}
          </div>
        </div>
      ),
    },
    {
      key: "mode",
      header: "Type",
      className: "whitespace-nowrap",
      render: (segment) => (
        <span className={segmentModeBadgeClass(segment.mode)}>
          {segment.mode}
        </span>
      ),
    },
    {
      key: "format",
      header: "Format",
      className: "whitespace-nowrap",
      render: (segment) =>
        segment.format ? (
          <span className="badge badge-outline uppercase">
            {segment.format}
          </span>
        ) : (
          <span className="text-base-content/45">--</span>
        ),
    },
    {
      key: "effort_count",
      header: "Efforts",
      className: "whitespace-nowrap font-semibold",
      render: (segment) => segment.effort_count,
    },
    {
      key: "distance_meters",
      header: "Distance",
      className: "whitespace-nowrap",
      render: (segment) => formatDistance(segment.distance_meters, unitSystem),
    },
    {
      key: "best_duration_seconds",
      header: "KOM",
      className: "whitespace-nowrap",
      render: (segment) =>
        formatDuration(segment.best_duration_seconds ?? null),
    },
    {
      key: "current_user_pr_duration_seconds",
      header: "Your PR",
      className: "whitespace-nowrap",
      render: (segment) =>
        formatDuration(segment.current_user_pr_duration_seconds ?? null),
    },
  ];

  const useSegmentsGridQuery = (params: SegmentsGridParams) => {
    const normalizedParams = normalizeSegmentsGridParams(params);
    const filtered = filterSegments(segmentsQuery.data ?? [], normalizedParams);
    const startIndex = (normalizedParams.page - 1) * normalizedParams.per_page;

    return {
      data: filtered.slice(startIndex, startIndex + normalizedParams.per_page),
      isLoading: segmentsQuery.isLoading,
      raw: { metadata: { total: filtered.length } },
    };
  };

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
        result.processing_task_id
          ? `Imported ${result.title}. Segment matching queued as task ${result.processing_task_id}.`
          : `Imported ${result.title}. Segment matching queued.`,
      );
    } catch {
      // Mutation errors are surfaced by the app-level React Query handler.
    }
  };

  return (
    <section className="grid gap-6">
      <div className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="flex items-start justify-between gap-4">
            <div>
              <div className="mb-3 flex items-center gap-2 text-xs font-medium uppercase tracking-[0.24em] text-base-content/50">
                <h2>Import Segment</h2>
                <InfoTooltip
                  label="Build or import segments details"
                  tip={SEGMENT_BUILDER_HELP_TEXT}
                />
              </div>
              <p className="mt-2 max-w-2xl text-sm text-base-content/70">
                You can download Strava segment GPX files from
                <a
                  href="https://www.doogal.co.uk/SegmentExplorer"
                  target="_blank"
                >
                  Doogal Segment explorer
                </a>
                .
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

            {selectedFile && (
              <div className="card bg-base-100 shadow-sm">
                <div className="card-body p-4 text-sm text-base-content/70">
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div className="font-medium text-base-content">
                      {selectedFile.name}
                    </div>
                    <span className="badge badge-neutral badge-outline uppercase">
                      {getExtension(selectedFile.name) || "unknown"}
                    </span>
                  </div>
                </div>
              </div>
            )}

            <div className="mt-4 flex flex-wrap items-center gap-3">
              <button
                type="button"
                className="btn btn-primary"
                disabled={!selectedFile || uploadMutation.isPending}
                onClick={onUpload}
              >
                {uploadMutation.isPending ? "Importing..." : "Import segment"}
              </button>
              <InfoTooltip
                label="Segment route file details"
                tip="GPX and TCX work best because they keep route coordinates explicit."
              />
            </div>
          </fieldset>
        </div>
      </div>

      <MemoryRouter>
        <GenericList
          title={
            <span className="text-xs font-medium uppercase tracking-[0.24em] text-base-content/50">
              Segments
            </span>
          }
          actions={
            segmentsQuery.isFetching ? <LoadingSpinner size="xs" /> : undefined
          }
          paramsSchema={SegmentsGridSchema}
          useQuery={useSegmentsGridQuery}
          columns={segmentColumns}
          renderFilters={(params, setFilter) => (
            <>
              <input
                type="search"
                placeholder="Search segments"
                className="input input-sm input-bordered w-52"
                value={params.q ?? ""}
                onChange={(event) => {
                  setFilter("q", event.target.value);
                }}
              />
              <select
                className="select select-sm select-bordered w-36"
                value={params.mode ?? ""}
                onChange={(event) => {
                  setFilter("mode", event.target.value);
                }}
              >
                <option value="">All types</option>
                <option value="xc">XC</option>
                <option value="dh">DH</option>
              </select>
            </>
          )}
          emptyMessage="No segments found matching the current filters."
        />
      </MemoryRouter>
    </section>
  );
}
