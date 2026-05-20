"use client";

import { faCrown, faMedal, faRoute } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
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
}: SegmentDetailHeaderProps) {
  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body gap-6">
        <div className="flex flex-wrap items-start justify-between gap-6">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-base-content/45">
              FMR / Effort Comparison
            </p>
            <h1 className="mt-2 text-4xl font-semibold tracking-tight">
              {segment.title}
            </h1>
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
  );
}
