import {
  formatDuration,
  formatHeartRate,
  formatPower,
} from "../lib/activityFormatting";
import { type ActivityHeartRateZone } from "../lib/queries";

const ZONE_BAR_CLASS_NAMES = [
  "bg-sky-500",
  "bg-emerald-500",
  "bg-lime-500",
  "bg-amber-500",
  "bg-rose-500",
];

function formatHeartRateZoneRange(zone: ActivityHeartRateZone) {
  if (zone.min_bpm == null && zone.max_bpm == null) {
    return "Range unavailable";
  }

  if (zone.min_bpm == null) {
    return `Up to ${formatHeartRate(zone.max_bpm)}`;
  }

  if (zone.max_bpm == null) {
    return `Above ${formatHeartRate(zone.min_bpm - 1)}`;
  }

  return `${formatHeartRate(zone.min_bpm)} to ${formatHeartRate(zone.max_bpm)}`;
}

function formatSharePercent(value: number) {
  return Number.isInteger(value)
    ? `${value.toFixed(0)}%`
    : `${value.toFixed(1)}%`;
}

function zoneBarClassName(index: number) {
  return ZONE_BAR_CLASS_NAMES[index % ZONE_BAR_CLASS_NAMES.length];
}

export default function TrainingProfileSnapshot({
  estimatedFtpWatts,
  heartRateZones,
}: {
  estimatedFtpWatts: number | null | undefined;
  heartRateZones: ActivityHeartRateZone[] | null | undefined;
}) {
  const zones = heartRateZones ?? [];

  if (estimatedFtpWatts == null && zones.length === 0) {
    return null;
  }

  return (
    <>
      {zones.length > 0 ? (
        <div className="mt-4 rounded-box border border-base-300 bg-base-100 px-4 py-4">
          <div className="mb-3 flex items-center justify-between gap-3 text-xs font-medium uppercase tracking-[0.24em] text-base-content/50">
            <span>Zone distribution</span>
            <span>{zones.length} stored zones</span>
          </div>
          <div className="space-y-3">
            {zones.map((zone, index) => (
              <div
                key={zone.zone}
                className="grid gap-2 sm:grid-cols-[minmax(0,12rem)_minmax(0,1fr)_auto] sm:items-center"
              >
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="badge badge-ghost badge-sm">
                      {zone.label}
                    </span>
                    <span className="truncate text-xs text-base-content/60">
                      {formatHeartRateZoneRange(zone)}
                    </span>
                  </div>
                </div>

                <div className="flex items-center gap-3">
                  <div className="h-3 flex-1 overflow-hidden rounded-full bg-base-200">
                    <div
                      className={`h-full rounded-full transition-[width] ${zoneBarClassName(index)}`}
                      style={{
                        width: `${Math.max(0, Math.min(zone.share_percent, 100))}%`,
                      }}
                    />
                  </div>
                  <span className="w-12 text-right text-xs font-medium text-base-content/60">
                    {formatSharePercent(zone.share_percent)}
                  </span>
                </div>

                <div className="text-sm font-medium text-base-content sm:min-w-24 sm:text-right">
                  {formatDuration(zone.duration_seconds)}
                </div>
              </div>
            ))}
          </div>
        </div>
      ) : (
        <div className="alert mt-4 bg-base-100 text-sm text-base-content/75">
          Heart rate zones were not stored on this ride yet. Save your account
          zones and regenerate the upload to persist them.
        </div>
      )}
    </>
  );
}
