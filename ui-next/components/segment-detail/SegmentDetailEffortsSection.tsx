"use client";

import { Pagination } from "@ericbutera/kaleido";
import {
  faCrown,
  faMinus,
  faPlus,
  faRocket,
  faTrophy,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import {
  formatActivityTimestamp,
  formatDuration,
} from "../../lib/activityFormatting";
import type { Segment, SegmentEffort } from "../../lib/queries";
import { primarySegmentAchievement } from "../../lib/segmentAchievements";
import {
  EFFORTS_PER_PAGE,
  EFFORT_TIME_FILTERS,
  segmentEffortDayAttemptSummaries,
  type EffortTimeFilter,
  type SelectedEffortRow,
} from "../../lib/segmentDetail";

type SegmentDetailEffortsSectionProps = {
  segment: Segment;
  visibleEfforts: SegmentEffort[];
  selectedEffortIds: number[];
  selectedRows: SelectedEffortRow[];
  overallRankByEffortId: Map<number, number>;
  currentUserPr: SegmentEffort | null;
  effortTimeFilter: EffortTimeFilter;
  onEffortTimeFilterChange: (filter: EffortTimeFilter) => void;
  onAddEffort: (effortId: number) => void;
  onRemoveEffort: (effortId: number) => void;
};

export default function SegmentDetailEffortsSection({
  segment,
  visibleEfforts,
  selectedEffortIds,
  selectedRows,
  overallRankByEffortId,
  currentUserPr,
  effortTimeFilter,
  onEffortTimeFilterChange,
  onAddEffort,
  onRemoveEffort,
}: SegmentDetailEffortsSectionProps) {
  const [page, setPage] = useState(1);
  const selectedEffortIdSet = new Set(selectedEffortIds);
  const selectedRowByEffortId = new Map(
    selectedRows.map((row) => [row.effort.id, row]),
  );
  const totalPages = Math.max(
    1,
    Math.ceil(visibleEfforts.length / EFFORTS_PER_PAGE),
  );

  useEffect(() => {
    setPage(1);
  }, [segment.id, effortTimeFilter]);

  useEffect(() => {
    setPage((currentPage) => Math.min(currentPage, totalPages));
  }, [totalPages]);

  const paginatedEfforts = useMemo(() => {
    const startIndex = (page - 1) * EFFORTS_PER_PAGE;
    return visibleEfforts.slice(startIndex, startIndex + EFFORTS_PER_PAGE);
  }, [page, visibleEfforts]);
  const attemptSummaryByEffortId = useMemo(
    () => segmentEffortDayAttemptSummaries(visibleEfforts),
    [visibleEfforts],
  );
  const showAttemptColumn = Array.from(
    attemptSummaryByEffortId.values(),
  ).some((summary) => summary.attemptCount > 1);
  const rangeStart =
    visibleEfforts.length === 0 ? 0 : (page - 1) * EFFORTS_PER_PAGE + 1;
  const rangeEnd = Math.min(page * EFFORTS_PER_PAGE, visibleEfforts.length);

  return (
    <div className="card border border-base-300 bg-base-100 shadow-xl">
      <div className="card-body">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <h2 className="card-title text-xl">Efforts</h2>
        </div>

        <div className="mt-5 min-w-0 border border-base-300 bg-base-200 p-4">
          <div className="flex flex-col gap-3">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="text-sm text-base-content/70">
                {visibleEfforts.length === 0
                  ? "No efforts in this time window"
                  : `Showing ${rangeStart}-${rangeEnd} of ${visibleEfforts.length} efforts`}
              </div>
              <div className="join">
                {EFFORT_TIME_FILTERS.map((filter) => (
                  <button
                    key={filter.key}
                    type="button"
                    className={`join-item btn btn-sm ${effortTimeFilter === filter.key ? "btn-neutral" : "btn-ghost"}`}
                    onClick={() => {
                      onEffortTimeFilterChange(filter.key);
                    }}
                  >
                    {filter.label}
                  </button>
                ))}
              </div>
            </div>
          </div>

          {paginatedEfforts.length > 0 ? (
            <div aria-label="Segment efforts table" className="mt-5 space-y-4">
              <div className="overflow-x-auto border border-base-300 bg-base-100">
                <table className="table table-pin-rows table-sm">
                  <thead>
                    <tr>
                      <th className="w-14">Place</th>
                      <th className="w-20">
                        <span className="sr-only">Compare</span>
                      </th>
                      <th>Time</th>
                      {showAttemptColumn ? <th className="w-24">Run</th> : null}
                      <th>Rider</th>
                      <th>Date</th>
                    </tr>
                  </thead>
                  <tbody>
                    {paginatedEfforts.map((effort) => {
                      const checked = selectedEffortIdSet.has(effort.id);
                      const selectedRow = selectedRowByEffortId.get(effort.id);
                      const overallRank =
                        overallRankByEffortId.get(effort.id) ?? null;
                      const isCurrentUserPr = currentUserPr?.id === effort.id;
                      const attemptSummary = attemptSummaryByEffortId.get(
                        effort.id,
                      );
                      const achievement = primarySegmentAchievement({
                        overallRank,
                        personalRank: isCurrentUserPr ? 1 : null,
                        isFastestOfDay:
                          attemptSummary?.isFastestOfDay ?? false,
                      });
                      const rowClassName =
                        achievement?.kind === "pr"
                          ? "bg-primary/10"
                          : achievement?.kind === "kom"
                            ? "bg-warning/10"
                            : achievement?.kind === "fastest"
                              ? "bg-success/10"
                              : checked
                                ? "bg-base-200/70"
                                : undefined;

                      return (
                        <tr key={effort.id} className={rowClassName}>
                          <td className="font-mono text-sm font-semibold tabular-nums text-base-content/70">
                            {overallRank ?? "--"}
                          </td>
                          <td>
                            {checked ? (
                              <div className="flex items-center gap-1.5">
                                {selectedRow ? (
                                  <span
                                    aria-hidden
                                    className="inline-flex h-5 w-5 items-center justify-center text-[0.65rem] font-semibold text-white"
                                    style={{
                                      backgroundColor: selectedRow.color,
                                    }}
                                  >
                                    {selectedRow.markerLabel}
                                  </span>
                                ) : null}
                                <button
                                  type="button"
                                  className="btn btn-ghost btn-xs btn-circle"
                                  aria-label={`Remove ${effort.activity_title} from comparison`}
                                  onClick={() => {
                                    onRemoveEffort(effort.id);
                                  }}
                                >
                                  <FontAwesomeIcon
                                    icon={faMinus}
                                    className="h-3.5 w-3.5"
                                  />
                                  <span className="sr-only">
                                    Remove from comparison
                                  </span>
                                </button>
                              </div>
                            ) : (
                              <button
                                type="button"
                                className="btn btn-ghost btn-xs btn-circle"
                                aria-label={`Add ${effort.activity_title} to comparison`}
                                onClick={() => {
                                  onAddEffort(effort.id);
                                }}
                              >
                                <FontAwesomeIcon
                                  icon={faPlus}
                                  className="h-3.5 w-3.5"
                                />
                                <span className="sr-only">
                                  Add to comparison
                                </span>
                              </button>
                            )}
                          </td>
                          <td className="font-semibold text-base-content">
                            <div className="flex flex-wrap items-center gap-2">
                              <Link
                                href={`/activities/${effort.activity_id}`}
                                className="transition hover:text-primary"
                                title={effort.activity_title}
                              >
                                {formatDuration(effort.duration_seconds)}
                              </Link>
                              {achievement?.kind === "kom" ? (
                                <span className="badge badge-warning badge-xs gap-1">
                                  <FontAwesomeIcon
                                    icon={faCrown}
                                    className="h-3 w-3"
                                  />
                                  KOM
                                </span>
                              ) : achievement?.kind === "top-10" ? (
                                <span className="badge badge-warning badge-xs gap-1">
                                  <FontAwesomeIcon
                                    icon={faTrophy}
                                    className="h-3 w-3"
                                  />
                                  {achievement.longLabel}
                                </span>
                              ) : achievement?.kind === "pr" ? (
                                <span className="badge badge-primary badge-xs">
                                  PR
                                </span>
                              ) : achievement?.kind === "fastest" ? (
                                <span className="badge badge-success badge-xs gap-1">
                                  <FontAwesomeIcon
                                    icon={faRocket}
                                    className="h-3 w-3"
                                  />
                                  Fastest
                                </span>
                              ) : null}
                            </div>
                          </td>
                          {showAttemptColumn ? (
                            <td className="whitespace-nowrap text-xs font-medium text-base-content/70">
                              {attemptSummary && attemptSummary.attemptCount > 1
                                ? `Run ${attemptSummary.attemptNumber}`
                                : "--"}
                            </td>
                          ) : null}
                          <td>{effort.rider_name}</td>
                          <td className="whitespace-nowrap text-base-content/65">
                            {formatActivityTimestamp(
                              effort.activity_started_at,
                            )}
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>

              <Pagination
                page={page}
                perPage={EFFORTS_PER_PAGE}
                total={visibleEfforts.length}
                onPageChange={setPage}
              />
            </div>
          ) : (
            <div className="alert mt-5">
              <span>No efforts match this time window.</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
