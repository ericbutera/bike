import type { components } from "./openapi/react-query/api";
import { parseActivitySport } from "./activitySports";

export type HeatmapMetadata = components["schemas"]["HeatmapMetadata"];
export type HeatmapQuery = components["schemas"]["HeatmapQuery"];
export type HeatmapZones = components["schemas"]["HeatmapZones"];

function calendarDate(date: Date, timeZone: string): string {
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(date);
  const part = (key: string) => parts.find((p) => p.type === key)?.value;
  return `${part("year")}-${part("month")}-${part("day")}`;
}

function parseDay(day: string): Date {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(day))
    throw new Error("Choose valid calendar dates.");
  const date = new Date(`${day}T00:00:00Z`);
  if (
    !Number.isFinite(date.getTime()) ||
    date.toISOString().slice(0, 10) !== day
  )
    throw new Error("Choose valid calendar dates.");
  return date;
}

export function nextCalendarDay(day: string, offset = 1): string {
  const date = parseDay(day);
  date.setUTCDate(date.getUTCDate() + offset);
  return date.toISOString().slice(0, 10);
}

/** Find the first instant of a calendar date in an IANA timezone, including
 * midnight offset changes. This makes inclusive end dates correct across DST. */
export function calendarDayStart(day: string, timeZone: string): string {
  const center = parseDay(day).getTime();
  let low = center - 36 * 60 * 60 * 1000;
  let high = center + 36 * 60 * 60 * 1000;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (calendarDate(new Date(middle), timeZone) < day) low = middle + 1;
    else high = middle;
  }
  if (calendarDate(new Date(low), timeZone) !== day)
    throw new Error("That calendar date does not exist in this timezone.");
  return new Date(low).toISOString();
}

export function parseHeatmapFilters(
  params: URLSearchParams,
  browserTimeZone: string,
) {
  const start = params.get("start") ?? "";
  const end = params.get("end") ?? "";
  const sportValue = params.get("sport") ?? "";
  const timeZone = params.get("tz") || browserTimeZone;
  const sport = parseActivitySport(sportValue);
  let query: HeatmapQuery = {};
  let error: string | undefined;
  try {
    new Intl.DateTimeFormat("en", { timeZone }).format();
    if (sportValue && !sport)
      throw new Error("Choose a supported activity type.");
    if (start && end && start > end)
      throw new Error("Start date must be on or before end date.");
    query = {
      ...(sport ? { sport } : {}),
      ...(start ? { from: calendarDayStart(start, timeZone) } : {}),
      ...(end ? { to: calendarDayStart(nextCalendarDay(end), timeZone) } : {}),
    };
  } catch (cause) {
    error =
      cause instanceof RangeError
        ? "Choose a valid timezone."
        : cause instanceof Error
          ? cause.message
          : "Choose valid filters.";
  }
  return { start, end, sport: sportValue, timeZone, query, error };
}

export function heatmapTileUrl(metadata: HeatmapMetadata): string {
  const params = new URLSearchParams({
    revision: metadata.revision,
    style: metadata.style_version,
  });
  if (metadata.filters.sport) params.set("sport", metadata.filters.sport);
  if (metadata.filters.from) params.set("from", metadata.filters.from);
  if (metadata.filters.to) params.set("to", metadata.filters.to);
  return `/heatmap-tiles/{z}/{x}/{y}.png?${params}`;
}

export function heatmapPreset(
  preset: "all" | "year" | "month",
  timeZone: string,
  now = new Date(),
) {
  const today = calendarDate(now, timeZone);
  return preset === "all"
    ? { start: "", end: "" }
    : {
        start:
          preset === "year"
            ? `${today.slice(0, 4)}-01-01`
            : nextCalendarDay(today, -29),
        end: today,
      };
}
