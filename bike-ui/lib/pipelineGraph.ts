import type { components } from "./openapi/react-query/api";
export type PipelineGraph = components["schemas"]["PipelineGraph"];
export type PipelineTask = components["schemas"]["PipelineTask"];
const label = (value: string) => value.replace(/[<>\[\]"&\r\n\\]/g, " ");
const stageId = (task: number, attempt: number, stage: string) =>
  `stage_${task}_${attempt}_${stage.replace(/[^a-zA-Z0-9_]/g, "_")}`;
export type GatewayStep = {
  kind: string;
  started_at: string;
  finished_at: string;
  attempts?: number;
};
export function gatewaySteps(run: PipelineGraph): GatewayStep[] {
  if (!Array.isArray(run.gateway_history)) return [];
  return run.gateway_history.filter(
    (value): value is GatewayStep =>
      typeof value === "object" &&
      value !== null &&
      "kind" in value &&
      typeof value.kind === "string" &&
      "started_at" in value &&
      typeof value.started_at === "string" &&
      "finished_at" in value &&
      typeof value.finished_at === "string",
  );
}
/** Gateway intervals can overlap. Task edges use persisted parents; inline stages
 * use the import owner's dependencies, never a guessed chronological chain. */
export function pipelineMermaid(run: PipelineGraph): string {
  const lines = [
    "flowchart TD",
    `receipt["${label(run.entrypoint)} received"]`,
    'accepted["API durable acceptance"]',
    "receipt --> accepted",
  ];
  gatewaySteps(run).forEach((step, index) => {
    lines.push(
      `gateway_${index}["${label(step.kind)}"]`,
      `receipt --> gateway_${index}`,
      `gateway_${index} --> accepted`,
    );
  });
  const ids = new Set(run.tasks.map((task) => task.id));
  for (const task of run.tasks) taskNodes(lines, task, ids);
  run.outputs.forEach((output, index) => {
    lines.push(
      `output_${index}["${label(output.kind)} #${output.target_id} · ${label(output.status)}"]`,
      `accepted -. required .-> output_${index}`,
    );
    for (const task of run.tasks) {
      task.work.forEach((work, workIndex) => {
        if (
          work.kind === output.kind &&
          work.revision === output.revision &&
          work.outcome === "published" &&
          work.work_key.endsWith(`:${output.target_id}`)
        )
          lines.push(`work_${task.id}_${workIndex} --> output_${index}`);
      });
      for (const imported of task.imports) {
        if (
          output.kind === "activity" &&
          imported.activity_id === output.target_id &&
          output.status === "available"
        ) {
          const saved = imported.stages.find(
            (stage) => stage.stage === "activity_saved" && stage.completed_at,
          );
          if (saved)
            lines.push(
              `${stageId(task.id, imported.attempt_id, saved.stage)} -. activity lineage .-> output_${index}`,
            );
        }
      }
    }
  });
  return lines.join("\n");
}
function taskNodes(lines: string[], task: PipelineTask, ids: Set<number>) {
  lines.push(
    `task_${task.id}["#${task.id} ${label(task.task_type)} · ${label(task.status)}"]`,
  );
  let parent = "accepted";
  if (task.parent_task_id != null) {
    parent = `task_${task.parent_task_id}`;
    if (!ids.has(task.parent_task_id))
      lines.push(
        `${parent}["Parent #${task.parent_task_id} outside this page"]`,
      );
  }
  lines.push(`${parent} --> task_${task.id}`);
  for (const attempt of task.attempts)
    lines.push(
      `try_${task.id}_${attempt.attempt}["Attempt ${attempt.attempt} · ${label(attempt.outcome)}"]`,
      `task_${task.id} --> try_${task.id}_${attempt.attempt}`,
    );
  task.work.forEach((work, index) =>
    lines.push(
      `work_${task.id}_${index}["${label(work.kind)} · ${label(work.outcome)}"]`,
      `try_${task.id}_${work.attempt} --> work_${task.id}_${index}`,
    ),
  );
  for (const imported of task.imports) {
    for (const stage of imported.stages) {
      const node = stageId(task.id, imported.attempt_id, stage.stage);
      lines.push(`${node}["${label(stage.stage)} · ${label(stage.status)}"]`);
      if (!imported.edges.some((edge) => edge.to === stage.stage))
        lines.push(`task_${task.id} --> ${node}`);
    }
    for (const edge of imported.edges)
      lines.push(
        `${stageId(task.id, imported.attempt_id, edge.from)} --> ${stageId(task.id, imported.attempt_id, edge.to)}`,
      );
  }
}
