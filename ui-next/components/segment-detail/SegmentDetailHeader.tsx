"use client";

import { faCrown, faMedal, faRoute } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import {
  formatActivityTimestamp,
  formatDistance,
  formatDuration,
  formatElevation,
  type UnitSystem,
} from "../../lib/activityFormatting";
import type { Segment, SegmentEffort } from "../../lib/queries";
import { formatGradePercent } from "../../lib/segmentDetail";

type SegmentDetailHeaderProps = {
  segment: Segment;
  routeDistanceMeters: number | null | undefined;
  routeGradePercent: number | null | undefined;
  routeNetElevationMeters: number | null | undefined;
  unitSystem: UnitSystem;
  currentUserPr: SegmentEffort | null;
  currentUserName: string | null;
  currentUserPrDurationSeconds: number | null;
  currentUserPrLabel: string;
  overallKom: SegmentEffort | null;
  isEditingTitle: boolean;
  draftTitle: string;
  isSavingTitle: boolean;
  isDeletingSegment: boolean;
  builderEditHref: string | null;
  actionErrorMessage: string | null;
  onStartEditingTitle: () => void;
  onCancelEditingTitle: () => void;
  onDraftTitleChange: (value: string) => void;
  onSaveTitle: () => void;
  onDeleteSegment: () => void;
};

export default function SegmentDetailHeader({
  segment,
  routeDistanceMeters,
  routeGradePercent,
  routeNetElevationMeters,
  unitSystem,
  currentUserPr,
  currentUserName,
  currentUserPrDurationSeconds,
  currentUserPrLabel,
  overallKom,
  isEditingTitle,
  draftTitle,
  isSavingTitle,
  isDeletingSegment,
  builderEditHref,
  actionErrorMessage,
  onStartEditingTitle,
  onCancelEditingTitle,
  onDraftTitleChange,
  onSaveTitle,
  onDeleteSegment,
}: SegmentDetailHeaderProps) {
  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body gap-6">
        <div className="flex flex-wrap items-start justify-between gap-6">
          <div className="min-w-0 flex-1">
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-base-content/45">
              FMR / Effort Comparison
            </p>

            {isEditingTitle ? (
              <form
                className="mt-2 flex max-w-3xl flex-col gap-3 sm:flex-row sm:items-end"
                onSubmit={(event) => {
                  event.preventDefault();
                  onSaveTitle();
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
                      onDraftTitleChange(event.target.value);
                    }}
                  />
                </label>

                <div className="flex gap-2">
                  <button
                    type="submit"
                    className="btn btn-primary btn-sm"
                    disabled={isSavingTitle}
                  >
                    {isSavingTitle ? "Saving..." : "Save"}
                  </button>
                  <button
                    type="button"
                    className="btn btn-ghost btn-sm"
                    onClick={onCancelEditingTitle}
                    disabled={isSavingTitle}
                  >
                    Cancel
                  </button>
                </div>
              </form>
            ) : (
              <h1 className="mt-2 text-4xl font-semibold tracking-tight">
                {segment.title}
              </h1>
            )}

            <div className="mt-4 flex flex-wrap items-center gap-4 text-sm text-base-content/70">
              <span className="inline-flex items-center gap-2">
                <FontAwesomeIcon
                  icon={faRoute}
                  className="h-3.5 w-3.5 text-base-content/40"
                />
                {formatDistance(
                  routeDistanceMeters ?? segment.distance_meters,
                  unitSystem,
                )}
              </span>
              <span>{formatGradePercent(routeGradePercent)}</span>
              <span>
                {formatElevation(
                  routeNetElevationMeters != null
                    ? Math.abs(routeNetElevationMeters)
                    : null,
                  unitSystem,
                )}{" "}
                elev
              </span>
            </div>
            <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-base-content/60">
              <span>
                Imported {formatActivityTimestamp(segment.created_at)} from{" "}
                {segment.source}
              </span>
              {segment.format ? (
                <span className="badge badge-outline uppercase">
                  {segment.format}
                </span>
              ) : null}
              <span className="badge badge-ghost">
                {segment.effort_count} efforts
              </span>
            </div>
          </div>

          <div className="flex min-w-[18rem] flex-col gap-3">
            <div className="flex flex-wrap justify-end gap-2">
              {!isEditingTitle && builderEditHref ? (
                <Link href={builderEditHref} className="btn btn-outline btn-sm">
                  Edit in builder
                </Link>
              ) : null}

              {!isEditingTitle ? (
                <button
                  type="button"
                  className="btn btn-outline btn-sm"
                  onClick={onStartEditingTitle}
                >
                  Rename
                </button>
              ) : null}

              <button
                type="button"
                className="btn btn-outline btn-error btn-sm"
                onClick={onDeleteSegment}
                disabled={isDeletingSegment}
              >
                {isDeletingSegment ? "Deleting..." : "Delete segment"}
              </button>
            </div>

            {actionErrorMessage ? (
              <div className="alert alert-error">{actionErrorMessage}</div>
            ) : null}

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
                  {currentUserPr?.rider_name ?? currentUserName ?? "You"}
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
