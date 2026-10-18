"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import type { AdminActivity, AdminUser } from "../../lib/queries";
import type { TaskTarget } from "../../lib/adminTasks";
import ActionMenu, { type ActionMenuItem } from "../ui/ActionMenu";
import AdminTaskDialog from "./AdminTaskDialog";
import {
  activityTaskOptions,
  userTaskOptions,
  type TaskOption,
} from "./taskActions";

export function ActivityTaskMenu({ activity }: { activity: AdminActivity }) {
  const router = useRouter();
  const nativeStrava =
    activity.source === "strava" || activity.source === "strava_sync";
  const target: TaskTarget = {
    kind: "activity",
    id: activity.id,
    userId: activity.user_id,
    label: `${activity.title} (#${activity.id})`,
  };
  return (
    <TaskMenu
      target={target}
      options={activityTaskOptions}
      disabledReason={(option) => {
        if (option.task === "reprocess" && nativeStrava)
          return "Use Resync from Strava for this activity";
        if (option.task === "reprocess" && !activity.activity_import_id)
          return "No retained import";
        if (option.task === "strava_resync" && !nativeStrava)
          return "Not a Strava activity";
      }}
      extraItems={
        activity.activity_import_id
          ? [
              {
                label: "View import details",
                onSelect: () =>
                  router.push(`/admin/imports/${activity.activity_import_id}`),
              },
            ]
          : []
      }
    />
  );
}

export function UserTaskMenu({ user }: { user: AdminUser }) {
  return (
    <TaskMenu
      target={{
        kind: "user",
        id: user.id,
        userId: user.id,
        label: `${user.name || user.email} (#${user.id})`,
      }}
      options={userTaskOptions}
    />
  );
}

function TaskMenu({
  target,
  options,
  disabledReason,
  extraItems = [],
}: {
  target: TaskTarget;
  options: TaskOption[];
  disabledReason?: (option: TaskOption) => string | undefined;
  extraItems?: ActionMenuItem[];
}) {
  const [selected, setSelected] = useState<TaskOption | null>(null);
  return (
    <div className="inline-flex" onClick={(event) => event.stopPropagation()}>
      <ActionMenu
        label={`Tasks for ${target.label}`}
        items={[
          ...options.map((option) => ({
            label: option.label,
            disabled: !!disabledReason?.(option),
            explanation: disabledReason?.(option),
            onSelect: () => setSelected(option),
          })),
          ...extraItems,
        ]}
      />
      {selected ? (
        <AdminTaskDialog
          target={target}
          option={selected}
          onClose={() => setSelected(null)}
        />
      ) : null}
    </div>
  );
}
