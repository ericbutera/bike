import type { AdminActivity, AdminUser } from "@/lib/queries";
import type { AcceptedTask, TaskTarget } from "@/lib/adminTasks";
import { acceptedReprocessing } from "./reprocessing";
import type { TaskExecution } from "@/lib/adminTaskHistory";

export function taskExecution(
  overrides: Partial<TaskExecution> = {},
): TaskExecution {
  return {
    job: {
      job_id: "bike-fitness-1",
      run_id: "run-1",
      namespace: "bike",
      workflow_type: "rebuild_fitness_freshness",
    },
    status: "Completed",
    started_at: "2026-10-17T12:00:00Z",
    closed_at: "2026-10-17T12:01:00Z",
    ...overrides,
  };
}

export function adminActivity(
  overrides: Partial<AdminActivity> = {},
): AdminActivity {
  return {
    id: 42,
    user_id: 7,
    title: "Lunch ride",
    sport: "road_ride",
    source: "manual_upload",
    started_at: "2026-10-10T12:00:00Z",
    activity_import_id: 17,
    ...overrides,
  };
}

export function adminUser(overrides: Partial<AdminUser> = {}): AdminUser {
  return {
    id: 7,
    name: "Rider",
    email: "rider@example.com",
    pid: "fixture-user",
    is_admin: false,
    disabled: false,
    email_verified: true,
    created_at: "2026-10-10T12:00:00Z",
    updated_at: "2026-10-10T12:00:00Z",
    ...overrides,
  };
}

export function taskTarget(overrides: Partial<TaskTarget> = {}): TaskTarget {
  return { kind: "user", id: 7, userId: 7, label: "Rider (#7)", ...overrides };
}

export function acceptedTask(
  overrides: Partial<AcceptedTask> = {},
): AcceptedTask {
  return {
    status: "accepted",
    user_id: 7,
    activity_id: 42,
    job: acceptedReprocessing().job,
    ...overrides,
  };
}
