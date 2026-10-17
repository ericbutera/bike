import type { ReprocessActivityImportResponse } from "@/lib/queries";

export function acceptedReprocessing(
  overrides: Partial<ReprocessActivityImportResponse> = {},
): ReprocessActivityImportResponse {
  const job = {
    job_id: "bike-task-reprocess_activity_import-user-1-fixture-command",
    namespace: "bike",
    run_id: "fixture-run",
    workflow_type: "reprocess_activity_import",
  };
  return {
    activity_id: 1647,
    activity_import_id: 88,
    user_id: 1,
    job,
    status: "accepted",
    message: "Activity reprocessing accepted.",
    task_id: job.job_id,
    task_status: "accepted",
    ...overrides,
  };
}
