"use client";

import { $typedApi } from "./api";
import { ADMIN_QUERY_OPTIONS } from "./adminQueryOptions";
import type { components } from "./openapi/react-query/api";

export type TaskExecution = components["schemas"]["TaskExecution"];
export type TaskStatus = components["schemas"]["TaskStatus"];

export function useTaskHistory(
  status?: TaskStatus,
  pageToken?: string,
  userId?: number,
  enabled = true,
) {
  return $typedApi.useQuery(
    "get",
    "/admin/tasks",
    {
      params: { query: { status, page_token: pageToken, user_id: userId } },
    },
    { ...ADMIN_QUERY_OPTIONS, enabled },
  );
}

export function useTaskInput(job: TaskExecution["job"]) {
  return $typedApi.useQuery(
    "get",
    "/admin/tasks/{workflow_id}/runs/{run_id}/input",
    { params: { path: { workflow_id: job.job_id, run_id: job.run_id } } },
    { ...ADMIN_QUERY_OPTIONS, staleTime: Infinity },
  );
}
