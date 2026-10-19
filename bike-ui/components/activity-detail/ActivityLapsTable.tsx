import {
  formatDistance,
  formatDuration,
  formatHeartRate,
  formatSpeed,
  type UnitSystem,
} from "../../lib/activityFormatting";
import type { ActivityLap } from "./types";

export default function ActivityLapsTable({
  laps,
  unitSystem,
}: {
  laps: ActivityLap[];
  unitSystem: UnitSystem;
}) {
  return (
    <div className="overflow-x-auto">
      <table className="table table-sm table-zebra" aria-label="Lap splits">
        <thead>
          <tr>
            <th scope="col">Lap</th>
            <th scope="col">Name</th>
            <th scope="col">Time</th>
            <th scope="col">Distance</th>
            <th scope="col">Avg speed</th>
            <th scope="col">Avg heart rate</th>
            <th scope="col">Max heart rate</th>
          </tr>
        </thead>
        <tbody>
          {laps.map((lap) => (
            <tr key={`${lap.lap_index}-${lap.title}`}>
              <th scope="row">{lap.lap_index}</th>
              <td>{lap.title}</td>
              <td className="whitespace-nowrap tabular-nums">
                {formatDuration(lap.duration_seconds)}
              </td>
              <td className="whitespace-nowrap tabular-nums">
                {formatDistance(lap.distance_meters, unitSystem)}
              </td>
              <td className="whitespace-nowrap tabular-nums">
                {formatSpeed(lap.average_speed_mps, unitSystem)}
              </td>
              <td className="whitespace-nowrap tabular-nums">
                {formatHeartRate(lap.average_heart_rate_bpm)}
              </td>
              <td className="whitespace-nowrap tabular-nums">
                {formatHeartRate(lap.max_heart_rate_bpm)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
