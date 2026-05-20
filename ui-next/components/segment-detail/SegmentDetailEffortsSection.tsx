"use client";

import {
  faCrown,
  faMagnifyingGlass,
  faMinus,
  faPlus,
  faTrophy,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import {
  formatActivityTimestamp,
  formatDuration,
} from "../../lib/activityFormatting";
import type { Segment, SegmentEffort } from "../../lib/queries";
import { primarySegmentAchievement } from "../../lib/segmentAchievements";
import {
  EFFORTS_TABLE_MAX_HEIGHT_REM,
  EFFORTS_VISIBLE_ROWS,
  EFFORT_TIME_FILTERS,
  type EffortTimeFilter,
  type SelectedEffortRow,
} from "../../lib/segmentDetail";

type SegmentDetailEffortsSectionProps = {
  segment: Segment;
  filteredVisibleEfforts: SegmentEffort[];
  visibleEfforts: SegmentEffort[];
  selectedEffortIds: number[];
  selectedRows: SelectedEffortRow[];
  overallRankByEffortId: Map<number, number>;
  currentUserPr: SegmentEffort | null;
  effortTimeFilter: EffortTimeFilter;
  effortSearchQuery: string;
  comparisonSelectionLabel: string;
  onEffortTimeFilterChange: (filter: EffortTimeFilter) => void;
  onEffortSearchQueryChange: (query: string) => void;
  onAddEffort: (effortId: number) => void;
  onRemoveEffort: (effortId: number) => void;
};

export default function SegmentDetailEffortsSection({
  segment,
  filteredVisibleEfforts,
  visibleEfforts,
  selectedEffortIds,
  selectedRows,
  overallRankByEffortId,
  currentUserPr,
  effortTimeFilter,
  effortSearchQuery,
  comparisonSelectionLabel,
  onEffortTimeFilterChange,
  onEffortSearchQueryChange,
  onAddEffort,
  onRemoveEffort,
}: SegmentDetailEffortsSectionProps) {
  const selectedEffortIdSet = new Set(selectedEffortIds);
  const selectedRowByEffortId = new Map(
    selectedRows.map((row) => [row.effort.id, row]),
  );

  return (
    <div className="card border border-base-300 bg-base-100 shadow-xl">
      <div className="card-body">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h2 className="card-title text-xl">Efforts</h2>
            <p className="text-sm text-base-content/70">
              Select as many attempts as you want, then use time to open the
              full activity detail.
            </p>
          </div>
          <span className="badge badge-outline whitespace-nowrap">
            {comparisonSelectionLabel}
          </span>
        </div>

        <div className="mt-5 min-w-0 border border-base-300 bg-base-200 p-4">
          <div className="flex flex-col gap-3">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="text-sm text-base-content/70">
                {filteredVisibleEfforts.length} of{" "}
                {(segment.efforts ?? []).length} efforts
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

            <label className="input input-bordered flex items-center gap-2 bg-base-100">
              <FontAwesomeIcon
                icon={faMagnifyingGlass}
                className="h-4 w-4 text-base-content/50"
              />
              <input
                type="search"
                value={effortSearchQuery}
                onChange={(event) => {
                  onEffortSearchQueryChange(event.target.value);
                }}
                className="grow"
                placeholder="Search rides or riders"
                aria-label="Search efforts"
              />
            </label>

            {visibleEfforts.length > EFFORTS_VISIBLE_ROWS ? (
              <div className="text-xs text-base-content/55">
                Scroll to see more than {EFFORTS_VISIBLE_ROWS} efforts
              </div>
            ) : null}
          </div>

          {filteredVisibleEfforts.length > 0 ? (
            <div
              aria-label="Segment efforts table"
              className="mt-5 overflow-x-auto overflow-y-auto border border-base-300 bg-base-100"
              style={{ maxHeight: `${EFFORTS_TABLE_MAX_HEIGHT_REM}rem` }}
            >
              <table className="table table-pin-rows table-sm">
                <thead>
                  <tr>
                    <th className="w-14">Place</th>
                    <th className="w-20">
                      <span className="sr-only">Compare</span>
                    </th>
                    <th>Time</th>
                    <th>Rider</th>
                    <th>Date</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredVisibleEfforts.map((effort) => {
                    const checked = selectedEffortIdSet.has(effort.id);
                    const selectedRow = selectedRowByEffortId.get(effort.id);
                    const overallRank =
                      overallRankByEffortId.get(effort.id) ?? null;
                    const isCurrentUserPr = currentUserPr?.id === effort.id;
                    const achievement = primarySegmentAchievement({
                      overallRank,
                      personalRank: isCurrentUserPr ? 1 : null,
                    });
                    const rowClassName =
                      achievement?.kind === "pr"
                        ? "bg-primary/10"
                        : achievement?.kind === "kom"
                          ? "bg-warning/10"
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
                                  style={{ backgroundColor: selectedRow.color }}
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
                              <span className="sr-only">Add to comparison</span>
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
                            ) : null}
                          </div>
                        </td>
                        <td>{effort.rider_name}</td>
                        <td className="whitespace-nowrap text-base-content/65">
                          {formatActivityTimestamp(effort.activity_started_at)}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="alert mt-5">
              <span>
                {effortSearchQuery.trim().length > 0
                  ? "No efforts match this search."
                  : "No efforts match this time window."}
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
