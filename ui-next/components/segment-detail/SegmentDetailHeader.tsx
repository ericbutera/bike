"use client";

import {
  faBars,
  faCrown,
  faMedal,
  faRoute,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import {
  formatActivityTimestamp,
  formatDistance,
  formatDuration,
  formatElevation,
} from "../../lib/activityFormatting";
import type { Segment, SegmentMode } from "../../lib/queries";
import { formatGradePercent } from "../../lib/segmentDetail";
import type {
  SegmentPerformanceSummary,
  SegmentRouteMetrics,
  SegmentTitleEditor,
} from "./useSegmentDetailState";

type SegmentDetailHeaderProps = {
  segment: Segment;
  metrics: SegmentRouteMetrics;
  performance: SegmentPerformanceSummary;
  editor: SegmentTitleEditor;
  status: {
    isSavingSegment: boolean;
    isDeletingSegment: boolean;
  };
  links: {
    builderEditHref: string | null;
  };
  actions: {
    changeSegmentMode: (mode: SegmentMode) => void;
    deleteSegment: () => void;
  };
};

export default function SegmentDetailHeader({
  segment,
  metrics,
  performance,
  editor,
  status,
  links,
  actions,
}: SegmentDetailHeaderProps) {
  const { routeDistanceMeters, routeGradePercent, routeNetElevationMeters } =
    metrics;
  const {
    currentUserPrDurationSeconds,
    currentUserPrLabel,
    currentUserPrDisplayName,
    overallKom,
  } = performance;
  const { isEditingTitle, draftTitle } = editor;
  const { isSavingSegment, isDeletingSegment } = status;

  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body gap-6">
        <div className="flex flex-wrap items-start justify-between gap-6">
          <div className="min-w-0 flex-1">
            {isEditingTitle ? (
              <form
                className="flex max-w-3xl flex-col gap-3 sm:flex-row sm:items-end"
                onSubmit={(event) => {
                  event.preventDefault();
                  editor.saveTitle();
                }}
              >
                <label className="form-control w-full">
                  <div className="label px-0 pb-2">
                    <span className="label-text font-medium text-base-content/70">
                      Segment name
                    </span>
                  </div>
                  <input
                    type="text"
                    className="input input-bordered input-lg w-full bg-base-100"
                    value={draftTitle}
                    autoFocus
                    onChange={(event) => {
                      editor.setDraftTitle(event.target.value);
                    }}
                  />
                </label>

                <div className="flex gap-2">
                  <button
                    type="submit"
                    className="btn btn-primary btn-sm"
                    disabled={isSavingSegment}
                  >
                    {isSavingSegment ? "Saving..." : "Save"}
                  </button>
                  <button
                    type="button"
                    className="btn btn-ghost btn-sm"
                    onClick={editor.cancelEditingTitle}
                    disabled={isSavingSegment}
                  >
                    Cancel
                  </button>
                </div>
              </form>
            ) : (
              <h1 className="text-4xl font-semibold tracking-tight">
                {segment.title}
              </h1>
            )}

            <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-2 text-sm text-base-content/70">
              <span className="inline-flex items-center gap-2">
                <FontAwesomeIcon
                  icon={faRoute}
                  className="h-3.5 w-3.5 text-base-content/40"
                />
                {formatDistance(
                  routeDistanceMeters ?? segment.distance_meters,
                  metrics.unitSystem,
                )}
              </span>
              <span>{formatGradePercent(routeGradePercent)}</span>
              <span>
                {formatElevation(
                  routeNetElevationMeters != null
                    ? Math.abs(routeNetElevationMeters)
                    : null,
                  metrics.unitSystem,
                )}{" "}
                elev
              </span>
              {segment.format ? (
                <span className="badge badge-outline uppercase">
                  {segment.format}
                </span>
              ) : null}
              <span
                className={`badge ${segment.mode === "dh" ? "badge-warning" : "badge-outline"}`}
              >
                {segment.mode.toUpperCase()} mode
              </span>
              <span className="badge badge-ghost">
                {segment.effort_count} efforts
              </span>
            </div>
            <div className="mt-2 flex flex-wrap items-center gap-2 text-xs text-base-content/60">
              <span>
                Imported {formatActivityTimestamp(segment.created_at)} from{" "}
                {segment.source}
              </span>
            </div>
          </div>

          <div className="flex min-w-[16rem] flex-col gap-3 sm:items-end">
            <div className="join self-start sm:self-end">
              <select
                aria-label="Segment mode"
                className="select select-bordered select-sm join-item min-w-[6rem] bg-base-100"
                value={segment.mode}
                disabled={isSavingSegment || isDeletingSegment}
                onChange={(event) => {
                  actions.changeSegmentMode(event.target.value as SegmentMode);
                }}
              >
                <option value="xc">XC</option>
                <option value="dh">DH</option>
              </select>

              <div className="dropdown dropdown-end">
                <button
                  type="button"
                  tabIndex={0}
                  className="btn btn-ghost btn-square btn-sm join-item border border-base-300 bg-base-100"
                  aria-label="Open segment actions"
                >
                  <FontAwesomeIcon icon={faBars} className="h-4 w-4" />
                </button>
                <ul
                  tabIndex={0}
                  className="dropdown-content menu z-20 mt-2 w-56 rounded-box border border-base-300 bg-base-100 p-2 shadow-lg"
                >
                  {links.builderEditHref ? (
                    <li>
                      <Link href={links.builderEditHref}>Edit in builder</Link>
                    </li>
                  ) : null}
                  {!isEditingTitle ? (
                    <li>
                      <button
                        type="button"
                        onClick={editor.startEditingTitle}
                      >
                        Rename
                      </button>
                    </li>
                  ) : null}
                  <li>
                    <button
                      type="button"
                      className="text-error"
                      onClick={actions.deleteSegment}
                      disabled={isDeletingSegment}
                    >
                      {isDeletingSegment ? "Deleting..." : "Delete segment"}
                    </button>
                  </li>
                </ul>
              </div>
            </div>

            <div className="grid gap-3 sm:grid-cols-2">
              <div className="min-w-[14rem] border border-base-300 bg-base-200 px-4 py-4">
                <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-base-content/55">
                  <FontAwesomeIcon
                    icon={faMedal}
                    className="h-3.5 w-3.5 text-primary"
                  />
                  <span>
                    Your PR {formatDuration(currentUserPrDurationSeconds)}
                  </span>
                </div>
                <div className="mt-2 font-semibold text-base-content">
                  {currentUserPrDisplayName}
                </div>
                <div className="mt-1 text-sm text-base-content/65">
                  {currentUserPrLabel}
                </div>
              </div>

              <div className="min-w-[14rem] border border-base-300 bg-base-200 px-4 py-4">
                <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-base-content/55">
                  <FontAwesomeIcon
                    icon={faCrown}
                    className="h-3.5 w-3.5 text-warning"
                  />
                  <span>
                    KOM {formatDuration(overallKom?.duration_seconds ?? null)}
                  </span>
                </div>
                <div className="mt-2 font-semibold text-base-content">
                  {overallKom?.rider_name ?? "No efforts yet"}
                </div>
                <div className="mt-1 text-sm text-base-content/65">
                  {overallKom?.activity_title ??
                    "Waiting for the first matched effort"}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
