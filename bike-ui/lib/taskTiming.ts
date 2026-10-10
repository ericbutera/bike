export function millisecondsBetween(
  start?: string | null,
  end?: string | null,
) {
  if (!start || !end) return null;
  const duration = new Date(end).getTime() - new Date(start).getTime();
  return Number.isFinite(duration) && duration >= 0 ? duration : null;
}

export function formatTaskDate(value?: string | null) {
  if (!value) return "";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

export function formatTaskDuration(value: number | null) {
  if (value === null) return "";
  const seconds = Math.max(0, Math.round(value / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours) return String(hours) + "h " + String(minutes) + "m";
  if (minutes) return String(minutes) + "m " + String(seconds % 60) + "s";
  return String(seconds) + "s";
}
