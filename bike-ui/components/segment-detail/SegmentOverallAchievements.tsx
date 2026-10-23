"use client";

import { formatDuration } from "../../lib/activityFormatting";
import {
  useSegmentOverallAchievements,
  type OverallAchievement,
} from "../../lib/queries";
import { DataGrid, ListCard, type Column } from "../ui/DataGrid";

const columns: Column<OverallAchievement>[] = [
  { key: "position", header: "Position", render: (row) => row.position },
  {
    key: "athlete",
    header: "Athlete",
    render: (row) => row.rider_name ?? "Athlete",
  },
  {
    key: "time",
    header: "Time",
    render: (row) => formatDuration(row.duration_seconds),
  },
];

export default function SegmentOverallAchievements({
  segmentId,
}: {
  segmentId: number | string;
}) {
  const query = useSegmentOverallAchievements(segmentId);
  return (
    <ListCard title="Overall top ten">
      <p className="text-sm text-base-content/70">
        All time · Fastest effort per athlete
      </p>
      <DataGrid
        label="Overall top ten"
        columns={columns}
        rows={query.data ?? []}
        isLoading={query.isLoading}
        error={query.isError ? "Unable to load overall standings." : undefined}
        emptyMessage="No overall standings yet."
      />
    </ListCard>
  );
}
