"use client";

import { useState } from "react";
import { extractApiMessage } from "../../lib/activityFormatting";
import { useTrainingReports } from "../../lib/queries";
import TimeRangeSelector, { TimeRange } from "./TimeRangeSelector";
import Charts from "./Charts";

export default function ReportsClient() {
  const [range, setRange] = useState<TimeRange>("month");
  const { data, isLoading, isError, error, isFetching } = useTrainingReports(range);

  const points = data?.points ?? [];

  return (
    <div>
      <div className="flex items-center justify-between gap-4">
        <h1 className="text-2xl font-semibold">Reports</h1>
        <TimeRangeSelector value={range} onChange={setRange} />
      </div>

      {isLoading ? (
        <div className="alert mt-4">
          <span>Loading report data...</span>
        </div>
      ) : null}

      {isError ? (
        <div className="alert alert-error mt-4">
          <span>{extractApiMessage(error) || "Failed to load reports."}</span>
        </div>
      ) : null}

      {!isLoading && !isError && points.length === 0 ? (
        <div className="alert mt-4">
          <span>No report data found for this boundary.</span>
        </div>
      ) : null}

      <div className="grid grid-cols-1 gap-6 md:grid-cols-2 mt-4">
        <div className="card p-4">
          <h2 className="text-lg font-medium mb-2">Z2 Speed</h2>
          <Charts type="z2_speed" points={points} range={range} />
        </div>

        <div className="card p-4">
          <h2 className="text-lg font-medium mb-2">Decoupling</h2>
          <Charts type="decoupling" points={points} range={range} />
        </div>

        <div className="card p-4">
          <h2 className="text-lg font-medium mb-2">Climbing Pace (feet/week)</h2>
          <Charts type="climbing_pace" points={points} range={range} />
        </div>

        <div className="card p-4">
          <h2 className="text-lg font-medium mb-2">Heart Rate Zones</h2>
          <Charts type="hr_zones" points={points} range={range} />
        </div>

        <div className="card p-4 md:col-span-2">
          <h2 className="text-lg font-medium mb-2">Elevation</h2>
          <Charts type="elevation" points={points} range={range} />
        </div>
      </div>

      {!isLoading && !isError && isFetching ? (
        <p className="mt-3 text-sm opacity-70">Refreshing...</p>
      ) : null}
    </div>
  );
}
