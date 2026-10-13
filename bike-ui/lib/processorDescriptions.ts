const descriptions: Record<string, string> = {
  activity_archive_import:
    "Extracts activity files from an uploaded archive and queues each file for import, tracking progress and per-file results.",
  backfill_user_xc_training:
    "Rebuilds cross-country training analysis for a user's stored activities and tracks backfill progress.",
  email_notification:
    "Sends a queued email notification using the configured email provider.",
  prepare_heatmap:
    "Prepares filtered activity routes for the user's personal heatmap and publishes the current projection.",
  process_activity_import:
    "Parses a newly received activity file, saves its activity data, and schedules the downstream processing needed to make it available.",
  rebuild_fitness_freshness:
    "Rebuilds a user's fitness, fatigue, and freshness analytics from their activity history.",
  rebuild_segment_analytics:
    "Rebuilds cached segment analytics from the recorded segment efforts.",
  regenerate_activity_segments:
    "Finds segment matches and regenerates segment efforts for one activity.",
  regenerate_segment_efforts:
    "Recalculates activity efforts for a specific segment.",
  regenerate_user_segments:
    "Queues segment matching and effort regeneration across a user's activities in bounded batches.",
  reprocess_activity_import:
    "Reprocesses a retained activity import from its original source to repair or refresh the stored activity and derived data.",
  reprocess_archive_fit_activity_imports:
    "Queues reprocessing of a user's retained FIT imports from activity archives in bounded batches.",
  reprocess_user_activity_imports:
    "Queues reprocessing of a user's retained activity imports in bounded batches.",
};

export function processorDescription(taskType: string) {
  return Object.hasOwn(descriptions, taskType)
    ? descriptions[taskType]
    : "Runs background work registered under this processor type. A specific description is not available yet.";
}
