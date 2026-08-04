import {
  formatDistance,
  formatDuration,
  formatHeartRate,
  formatSpeed,
  type UnitSystem,
} from "../../lib/activityFormatting";
import { type ActivityLap } from "../../lib/queries";
import MetricCard from "../MetricCard";

export default function LapCard({
  lap,
  unitSystem,
}: {
  lap: ActivityLap;
  unitSystem: UnitSystem;
}) {
  return (
    <div className="card bg-base-200 shadow-sm">
      <div className="card-body p-5">
        <div className="flex items-start justify-between gap-3">
          <div>
            <p className="text-sm text-base-content/60">Lap {lap.lap_index}</p>
            <h3 className="card-title text-lg">{lap.title}</h3>
          </div>
          <span className="badge badge-outline">
            {formatDuration(lap.duration_seconds)}
          </span>
        </div>

        <div className="mt-4 grid gap-3 sm:grid-cols-2">
          <MetricCard
            label="Distance"
            value={formatDistance(lap.distance_meters, unitSystem)}
          />
          <MetricCard
            label="Average speed"
            value={formatSpeed(lap.average_speed_mps, unitSystem)}
          />
          <MetricCard
            label="Average heart rate"
            value={formatHeartRate(lap.average_heart_rate_bpm)}
          />
          <MetricCard
            label="Max heart rate"
            value={formatHeartRate(lap.max_heart_rate_bpm)}
          />
        </div>
      </div>
    </div>
  );
}
