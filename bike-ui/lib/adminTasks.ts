"use client";

import { useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { $typedApi } from "./api";
import type { components } from "./openapi/react-query/api";

export type ActivityAction = components["schemas"]["ActivityAction"];
export type UserAction = components["schemas"]["UserAction"];
export type AcceptedTask = components["schemas"]["AcceptedTask"];
export type TaskTarget = {
  kind: "activity" | "user";
  id: number;
  label: string;
  userId: number;
};
export type TaskParameters = { task: ActivityAction } | UserAction;

export function useAdminTask(target: TaskTarget) {
  const cache = useQueryClient();
  const activity = $typedApi.useMutation(
    "post",
    "/admin/activities/{id}/tasks",
    { onSettled: refreshHistory },
  );
  const user = $typedApi.useMutation("post", "/admin/users/{id}/tasks", {
    onSettled: refreshHistory,
  });
  const intents = useRef(new Map<string, string>());

  function refreshHistory() {
    return cache.invalidateQueries({
      queryKey: ["get", "/admin/integration-events"],
    });
  }

  async function submit(parameters: TaskParameters) {
    const key = JSON.stringify([target.kind, target.id, parameters]);
    const commandId = intents.current.get(key) ?? crypto.randomUUID();
    intents.current.set(key, commandId);
    const result =
      target.kind === "activity"
        ? await activity.mutateAsync({
            params: { path: { id: target.id } },
            body: {
              command_id: commandId,
              task: parameters.task as ActivityAction,
            },
          })
        : await user.mutateAsync({
            params: { path: { id: target.id } },
            body: { ...(parameters as UserAction), command_id: commandId },
          });
    intents.current.delete(key);
    return result;
  }

  return { submit, isPending: activity.isPending || user.isPending };
}
