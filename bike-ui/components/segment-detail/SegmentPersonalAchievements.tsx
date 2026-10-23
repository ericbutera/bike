"use client";

import Link from "next/link";
import { formatDuration } from "../../lib/activityFormatting";
import {
  useSegmentPersonalAchievements,
  type PersonalAchievement,
} from "../../lib/queries";

const POSITION_LABELS: Record<number, string> = {
  1: "Your PR",
  2: "Second",
  3: "Third",
};

export default function SegmentPersonalAchievements({
  segmentId,
}: {
  segmentId: number;
}) {
  const query = useSegmentPersonalAchievements(segmentId);

  return (
    <section
      aria-label="Personal achievements"
      className="min-w-0 border border-base-300 bg-base-200 px-4 py-4 sm:min-w-[14rem]"
    >
      <h2 className="text-xs font-semibold uppercase tracking-[0.16em] text-base-content/55">
        Personal achievements
      </h2>
      <p className="mt-1 text-xs text-base-content/60">All-time</p>
      {query.data === undefined ? (
        query.isError ? (
          <p role="alert" className="mt-2 text-sm text-error">
            Unable to load personal achievements.
          </p>
        ) : (
          <p role="status" className="mt-2 text-sm text-base-content/65">
            Loading personal achievements...
          </p>
        )
      ) : (
        <PersonalPositions segmentId={segmentId} positions={query.data} />
      )}
    </section>
  );
}

function PersonalPositions({
  segmentId,
  positions,
}: {
  segmentId: number;
  positions: PersonalAchievement[];
}) {
  if (positions.length === 0) {
    return (
      <p className="mt-2 text-sm text-base-content/65">
        No personal achievements yet.
      </p>
    );
  }

  return (
    <ol className="mt-2 space-y-2">
      {positions.map((position) => (
        <li key={position.position}>
          <Link
            className="link link-hover flex flex-wrap justify-between gap-x-4 font-semibold"
            href={`/segments/${segmentId}?efforts=${position.effort_id}`}
          >
            <span>{POSITION_LABELS[position.position]}</span>
            <span className="tabular-nums">
              {formatDuration(position.duration_seconds)}
            </span>
          </Link>
        </li>
      ))}
    </ol>
  );
}
