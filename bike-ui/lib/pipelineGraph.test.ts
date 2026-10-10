import { describe, it, expect } from "vitest";
import {
  gatewaySteps,
  pipelineMermaid,
  type PipelineGraph,
  type PipelineTask,
} from "./pipelineGraph";
const root: PipelineGraph = {
  run_id: "run",
  entrypoint: "strava_webhook",
  received_at: "2026-10-10T12:00:00Z",
  accepted_at: "2026-10-10T12:01:00Z",
  outputs: [],
  tasks: [],
};
describe("pipelineMermaid", () => {
  it("keeps overlapping gateway intervals as siblings and ignores malformed history", () => {
    const history = [
      {
        kind: "gateway_receive_to_fetch",
        started_at: root.received_at,
        finished_at: root.accepted_at,
      },
      {
        kind: "gateway_delivery_wait",
        started_at: root.received_at,
        finished_at: root.accepted_at,
        attempts: 2,
      },
      { kind: "invalid", started_at: 4 },
      null,
    ];
    const run = { ...root, gateway_history: history };
    expect(gatewaySteps(run)).toHaveLength(2);
    expect(gatewaySteps({ ...root, gateway_history: {} })).toEqual([]);
    const graph = pipelineMermaid(run);
    expect(graph).toContain("receipt --> gateway_0");
    expect(graph).toContain("receipt --> gateway_1");
    expect(graph).not.toContain("gateway_0 --> gateway_1");
  });

  it("links only matching published work to a required output revision", () => {
    const work = {
      id: 1,
      task_id: 42,
      attempt: 1,
      kind: "heatmap",
      work_key: "activity:7",
      revision: "new",
      processing_version: "1",
      mode: "full",
      reason: "source_change",
      started_at: root.received_at,
      finished_at: root.accepted_at,
      outcome: "published",
    };
    const task: PipelineTask = {
      id: 42,
      task_type: "prepare_heatmap",
      status: "completed",
      created_at: root.accepted_at,
      attempts: [],
      imports: [],
      anomalies: [],
      work_has_more: false,
      work: [
        { ...work, id: 1, revision: "old" },
        { ...work, id: 2 },
        { ...work, id: 3, outcome: "superseded" },
      ],
    };
    const graph = pipelineMermaid({
      ...root,
      tasks: [task],
      outputs: [
        {
          run_id: root.run_id,
          kind: "heatmap",
          target_id: 7,
          revision: "new",
          required: true,
          status: "available",
          required_at: root.received_at,
        },
      ],
    });
    expect(graph).toContain("work_42_1 --> output_0");
    expect(graph).not.toContain("work_42_0 --> output_0");
    expect(graph).not.toContain("work_42_2 --> output_0");
  });

  it("uses canonical import dependencies and distinguishes lineage from publication", () => {
    const task: PipelineTask = {
      id: 42,
      task_type: "process_activity_import",
      status: "completed",
      created_at: root.accepted_at,
      attempts: [],
      work: [],
      anomalies: [],
      work_has_more: false,
      imports: [
        {
          attempt_id: 9,
          import_id: 8,
          activity_id: 7,
          status: "completed",
          stages: [
            { stage: "parsed", status: "completed", summary: [] },
            {
              stage: "activity_saved",
              status: "completed",
              summary: [],
              completed_at: root.accepted_at,
            },
          ],
          edges: [{ from: "parsed", to: "activity_saved" }],
        },
      ],
    };
    const graph = pipelineMermaid({
      ...root,
      tasks: [task],
      outputs: [
        {
          run_id: root.run_id,
          kind: "activity",
          target_id: 7,
          revision: "source",
          required: true,
          status: "available",
          required_at: root.received_at,
        },
      ],
    });
    expect(graph).toContain("task_42 --> stage_42_9_parsed");
    expect(graph).toContain("stage_42_9_parsed --> stage_42_9_activity_saved");
    expect(graph).toContain(
      "stage_42_9_activity_saved -. activity lineage .-> output_0",
    );
  });
  it("connects receipt, causal task handoffs and retry attempts", () => {
    const graph = pipelineMermaid({
      ...root,
      tasks: [
        {
          id: 42,
          task_type: "process_activity_import",
          status: "completed",
          created_at: root.accepted_at,
          work: [],
          anomalies: [],
          work_has_more: false,
          imports: [],
          attempts: [],
        },
        {
          id: 43,
          parent_task_id: 42,
          task_type: "rebuild_fitness_freshness",
          status: "processing",
          created_at: root.accepted_at,
          work: [],
          anomalies: [],
          work_has_more: false,
          imports: [],
          attempts: [
            {
              attempt: 2,
              started_at: root.accepted_at,
              heartbeat_at: root.accepted_at,
              progress_at: root.accepted_at,
              outcome: "running",
            },
          ],
        },
      ],
    });
    expect(graph).toContain("accepted --> task_42");
    expect(graph).toContain("task_42 --> task_43");
    expect(graph).toContain("task_43 --> try_43_2");
  });
  it("marks missing parents honestly and escapes untrusted labels", () => {
    const graph = pipelineMermaid({
      ...root,
      entrypoint: 'strava\" <script>',
      tasks: [
        {
          id: 43,
          parent_task_id: 42,
          task_type: "prepare_heatmap",
          status: "completed",
          created_at: root.accepted_at,
          work: [],
          anomalies: [],
          work_has_more: false,
          imports: [],
          attempts: [],
        },
      ],
    });
    expect(graph).toContain("Parent #42 outside this page");
    expect(graph).not.toContain("<script>");
  });
});
