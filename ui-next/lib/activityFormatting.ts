export const UNIT_SYSTEMS = ["metric", "imperial", "mixed"] as const;

export type UnitSystem = (typeof UNIT_SYSTEMS)[number];

export const DEFAULT_UNIT_SYSTEM: UnitSystem = "imperial";

export const METERS_PER_MILE = 1609.344;
export const FEET_PER_METER = 3.28084;
const MPH_PER_MPS = 2.236936;
const KPH_PER_MPS = 3.6;

export function normalizeUnitSystem(value?: string | null): UnitSystem {
  return value === "metric" || value === "imperial" || value === "mixed"
    ? value
    : DEFAULT_UNIT_SYSTEM;
}

export function formatActivityTimestamp(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleString(undefined, {
        dateStyle: "medium",
        timeStyle: "short",
      });
}

export function formatDistance(
  value?: number | null,
  unitSystem: UnitSystem = DEFAULT_UNIT_SYSTEM,
) {
  if (value == null) {
    return "--";
  }

  if (unitSystem === "imperial") {
    const miles = value / METERS_PER_MILE;
    return `${miles >= 100 ? miles.toFixed(0) : miles.toFixed(1)} mi`;
  }

  const kilometers = value / 1000;
  return `${kilometers >= 100 ? kilometers.toFixed(0) : kilometers.toFixed(1)} km`;
}

export function formatDuration(value?: number | null) {
  if (value == null || value <= 0) {
    return "--";
  }

  const hours = Math.floor(value / 3600);
  const minutes = Math.floor((value % 3600) / 60);
  const seconds = value % 60;

  if (hours > 0) {
    return `${hours}h ${String(minutes).padStart(2, "0")}m`;
  }

  if (minutes > 0) {
    return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
  }

  return `${seconds}s`;
}

export function formatElevation(
  value?: number | null,
  unitSystem: UnitSystem = DEFAULT_UNIT_SYSTEM,
) {
  if (value == null) {
    return "--";
  }

  if (unitSystem === "imperial") {
    return `${Math.round(value * FEET_PER_METER)} ft`;
  }

  return `${Math.round(value)} m`;
}

export function formatSpeed(
  value?: number | null,
  unitSystem: UnitSystem = DEFAULT_UNIT_SYSTEM,
) {
  if (value == null) {
    return "--";
  }

  if (unitSystem === "metric") {
    return `${(value * KPH_PER_MPS).toFixed(1)} km/h`;
  }

  return `${(value * MPH_PER_MPS).toFixed(1)} mph`;
}

export function formatHeartRate(value?: number | null) {
  if (value == null) {
    return "--";
  }

  return `${Math.round(value)} bpm`;
}

export function formatPower(value?: number | null) {
  if (value == null) {
    return "--";
  }

  return `${Math.round(value)} W`;
}

export function formatCadence(value?: number | null) {
  if (value == null) {
    return "--";
  }

  return `${Math.round(value)} rpm`;
}

export function formatCalories(value?: number | null) {
  if (value == null) {
    return "--";
  }

  return `${Math.round(value)} kcal`;
}

export function formatRelativeEffort(value?: number | null) {
  if (value == null) {
    return "--";
  }

  return `${Math.round(value)}`;
}

export function formatSport(value: string) {
  if (!value) {
    return "Activity";
  }

  return value.charAt(0).toUpperCase() + value.slice(1);
}

export function extractApiMessage(error: unknown) {
  if (typeof error === "object" && error && "message" in error) {
    const value = (error as { message?: unknown }).message;
    if (typeof value === "string") {
      return value;
    }
  }

  return "Request failed";
}
