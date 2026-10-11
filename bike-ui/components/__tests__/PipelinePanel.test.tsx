import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import PipelinePanel from "../admin/PipelinePanel";
import type { PipelineGraph } from "../../lib/pipelineGraph";
const { usePipelineGraph } = vi.hoisted(() => ({ usePipelineGraph: vi.fn() }));
vi.mock("../../lib/queries", () => ({ usePipelineGraph }));
vi.mock("../ui/MermaidFlowchart", () => ({
  default: ({ onSelect }: { onSelect: (node: string) => void }) => (
    <>
      <button onClick={() => onSelect("stage_42_9_activity_saved")}>
        Select saved stage
      </button>
      <button onClick={() => onSelect("try_42_2")}>Select retry attempt</button>
      <button onClick={() => onSelect("work_42_0")}>
        Select publication work
      </button>
    </>
  ),
}));
const run: PipelineGraph = {
  run_id: "run",
  entrypoint: "strava_webhook",
  received_at: "2026-10-10T12:00:00Z",
  accepted_at: "2026-10-10T12:00:10Z",
  available_at: "2026-10-10T12:01:00Z",
  ended_at: "2026-10-10T12:01:05Z",
  trace_id: "abcdef",
  grafana_url: "https://grafana.example.test",
  outputs: [],
  tasks: [
    {
      id: 42,
      task_type: "process_activity_import",
      status: "completed",
      created_at: "2026-10-10T12:00:10Z",
      attempts: [],
      work: [],
      work_has_more: false,
      anomalies: [],
      imports: [
        {
          attempt_id: 9,
          import_id: 8,
          activity_id: 7,
          status: "completed",
          edges: [],
          stages: [
            {
              stage: "activity_saved",
              status: "completed",
              started_at: "2026-10-10T12:00:20Z",
              completed_at: "2026-10-10T12:00:30Z",
              summary: ["1 activity saved"],
            },
          ],
        },
      ],
    },
  ],
};
describe("PipelinePanel", () => {
  it("uses the selected retry's timestamps instead of the first attempt", () => {
    const attempt = (
      number: number,
      start: string,
      finish: string,
      outcome: string,
    ) => ({
      attempt: number,
      started_at: start,
      finished_at: finish,
      outcome,
      heartbeat_at: start,
      progress_at: start,
    });
    usePipelineGraph.mockReturnValue({
      data: {
        ...run,
        tasks: [
          {
            ...run.tasks[0],
            attempts: [
              attempt(
                1,
                "2026-10-10T12:00:20Z",
                "2026-10-10T12:00:30Z",
                "retrying",
              ),
              attempt(
                2,
                "2026-10-10T12:00:40Z",
                "2026-10-10T12:01:00Z",
                "completed",
              ),
            ],
          },
        ],
      },
      isLoading: false,
      error: null,
    });
    render(<PipelinePanel runId="run" />);
    fireEvent.click(
      screen.getByRole("button", { name: "Select retry attempt" }),
    );
    const timing = within(
      screen.getByRole("region", { name: "Selected step timing" }),
    );
    expect(timing.getByText("20s")).toBeInTheDocument();
    expect(timing.getByText("40s")).toBeInTheDocument();
    expect(timing.getByText("1m 0s")).toBeInTheDocument();
  });
  it("shows receipt-to-publication and selected authoritative stage timings with trace links", () => {
    usePipelineGraph.mockReturnValue({
      data: run,
      isLoading: false,
      error: null,
    });
    render(<PipelinePanel runId="run" />);
    expect(
      screen.getByText("Receipt to availability").nextElementSibling,
    ).toHaveTextContent("1m 0s");
    expect(
      screen.getByRole("link", { name: "Open trace" }).getAttribute("href"),
    ).toContain("/explore?left=");
    fireEvent.click(screen.getByRole("button", { name: "Select saved stage" }));
    const timing = within(
      screen.getByRole("region", { name: "Selected step timing" }),
    );
    expect(timing.getByText("10s")).toBeInTheDocument();
    expect(timing.getByText("20s")).toBeInTheDocument();
    expect(timing.getByText("30s")).toBeInTheDocument();
    expect(
      screen.getByText("Receipt to queue").nextElementSibling,
    ).toHaveTextContent("10s");
    expect(
      screen.getByText("Receipt to first start").nextElementSibling,
    ).toHaveTextContent("N/A");
    expect(screen.getByText(/1 activity saved/)).toBeInTheDocument();
  });
  it("keeps missing readiness unknown", () => {
    usePipelineGraph.mockReturnValue({
      data: { ...run, available_at: null, ended_at: null, tasks: [] },
      isLoading: false,
      error: null,
    });
    render(<PipelinePanel runId="run" />);
    expect(
      screen.getByText("Receipt to availability").nextElementSibling,
    ).toHaveTextContent("N/A");
  });
  it("keeps measured work counts, publication readiness and anomaly evidence distinct", () => {
    usePipelineGraph.mockReturnValue({
      data: {
        ...run,
        outputs: [
          {
            kind: "heatmap",
            target_id: 7,
            status: "available",
            revision: "revision-3",
            required: true,
            required_at: run.received_at,
            available_at: run.available_at,
            run_id: run.run_id,
          },
        ],
        tasks: [
          {
            ...run.tasks[0],
            work: [
              {
                id: 10,
                task_id: 42,
                attempt: 1,
                kind: "heatmap",
                work_key: "heatmap:7",
                mode: "incremental",
                reason: "projection_changed",
                outcome: "published",
                revision: "revision-3",
                processing_version: "5",
                started_at: "2026-10-10T12:00:20Z",
                finished_at: "2026-10-10T12:00:35Z",
                counts: { tiles_written: 4, skipped: 0 },
                trace_id: "work-trace",
              },
            ],
            anomalies: [
              {
                task_id: 42,
                attempt: 1,
                reason: "duration_exceeded",
                observed: 15,
                expected: 10,
                baseline_samples: 30,
                policy_version: 1,
                severity: "warning",
                evaluated_at: run.ended_at!,
              },
            ],
          },
        ],
      },
      isLoading: false,
      error: null,
    });
    render(<PipelinePanel runId="run" />);
    fireEvent.click(
      screen.getByRole("button", { name: "Select publication work" }),
    );
    const timing = within(
      screen.getByRole("region", { name: "Selected step timing" }),
    );
    expect(timing.getByText("15s")).toBeInTheDocument();
    expect(
      screen.getByText("tiles written").nextElementSibling,
    ).toHaveTextContent("4");
    expect(screen.getByText("skipped").nextElementSibling).toHaveTextContent(
      "0",
    );
    expect(
      screen.getByText("Baseline samples").nextElementSibling,
    ).toHaveTextContent("30");
    const outputs = within(
      screen.getByRole("region", { name: "Published outputs" }),
    );
    expect(outputs.getByText("available")).toBeInTheDocument();
    expect(outputs.getByText("Revision").nextElementSibling).toHaveTextContent(
      "revision-3",
    );
    expect(usePipelineGraph).toHaveBeenLastCalledWith("run", 0, {
      work_task: 42,
      after_work: 0,
      output_offset: 0,
    });
  });
});
