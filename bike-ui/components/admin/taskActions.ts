import type { ActivityAction, TaskParameters } from "../../lib/adminTasks";

export type TaskOption = {
  task: TaskParameters["task"];
  label: string;
  description: string;
  fields?: "imports" | "dates";
};

export const activityTaskOptions: (TaskOption & { task: ActivityAction })[] = [
  {
    task: "reprocess",
    label: "Reprocess activity",
    description:
      "Read this activity's retained import again and rebuild its derived results. Your saved title and Training/Race selection are preserved.",
  },
  {
    task: "rebuild_fitness",
    label: "Rebuild fitness / freshness",
    description:
      "Rebuild fitness, fatigue and form for this activity's owner using all of their current activities.",
  },
  {
    task: "rebuild_segments",
    label: "Rebuild activity segments",
    description:
      "Rematch this activity to segments and refresh affected segment results and achievements.",
  },
  {
    task: "rebuild_heatmap",
    label: "Prepare activity heatmap",
    description:
      "Prepare this activity's pending heatmap projection using the current privacy and sport rules. A current projection may need no changes.",
  },
  {
    task: "strava_resync",
    label: "Resync activity from Strava",
    description:
      "Fetch the currently mapped Strava activity and refresh its source and derived results.",
  },
];

export const userTaskOptions: TaskOption[] = [
  {
    task: "rebuild_fitness",
    label: "Rebuild fitness / freshness",
    description:
      "Rebuild fitness, fatigue and form from this user's current activities.",
  },
  {
    task: "rebuild_segments",
    label: "Regenerate segment efforts",
    description:
      "Rematch this user's cycling activities and refresh segment results and achievements.",
  },
  {
    task: "reprocess_imports",
    label: "Reprocess user imports",
    description:
      "Reprocess this user's retained file imports. Native Strava activities are refreshed through Strava sync or backfill.",
    fields: "imports",
  },
  {
    task: "backfill_xc_training",
    label: "Backfill XC training",
    description:
      "Rebuild training analysis for this user's historical cycling activities.",
  },
  {
    task: "strava_sync",
    label: "Sync Strava",
    description:
      "Import new activities from this user's connected Strava account using its existing sync position.",
  },
  {
    task: "strava_backfill",
    label: "Backfill Strava",
    description:
      "Fetch activities from this user's connected Strava account within the selected dates, without changing the normal sync position.",
    fields: "dates",
  },
];
