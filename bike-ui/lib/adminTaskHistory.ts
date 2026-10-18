"use client";

import { $typedApi } from "./api";
import type { components } from "./openapi/react-query/api";

export type TaskExecution = components["schemas"]["TaskExecution"];
export type TaskStatus = components["schemas"]["TaskStatus"];

export function useTaskHistory(status?: TaskStatus, pageToken?: string) {
  return $typedApi.useQuery("get", "/admin/tasks", {
    params: { query: { status, page_token: pageToken } },
  });
}

export function useTaskInput(job: TaskExecution["job"]) {
  return $typedApi.useQuery(
    "get",
    "/admin/tasks/{workflow_id}/runs/{run_id}/input",
    { params: { path: { workflow_id: job.job_id, run_id: job.run_id } } },
    { staleTime: Infinity },
  );
}
