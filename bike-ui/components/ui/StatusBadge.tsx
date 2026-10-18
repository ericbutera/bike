const colors: Record<string, string> = {
  active: "badge-success",
  verified: "badge-success",
  success: "badge-success",
  complete: "badge-success",
  completed: "badge-success",
  processed: "badge-success",
  available: "badge-success",
  published: "badge-success",
  disabled: "badge-error",
  error: "badge-error",
  failed: "badge-error",
  terminated: "badge-error",
  timedout: "badge-error",
  info: "badge-info",
  running: "badge-info",
  processing: "badge-info",
  reused: "badge-info",
  continuedasnew: "badge-info",
  raw_stored: "badge-info",
  activity_parsed: "badge-info",
  activity_saved: "badge-info",
  segments_built: "badge-info",
  segment_analytics_built: "badge-info",
  activity_analytics_built: "badge-info",
  training_analysis_built: "badge-info",
  warning: "badge-warning",
  unverified: "badge-warning",
  duplicate: "badge-warning",
  queued: "badge-warning",
  retrying: "badge-warning",
  canceled: "badge-warning",
};

export default function StatusBadge({
  status,
  label = status,
  title,
}: {
  status: string;
  label?: string;
  title?: string;
}) {
  return (
    <span
      className={`badge whitespace-nowrap ${colors[status.toLowerCase()] ?? "badge-neutral"}`}
      title={title}
    >
      {label}
    </span>
  );
}
