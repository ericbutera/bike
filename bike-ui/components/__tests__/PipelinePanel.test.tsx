import { fireEvent, render, screen } from "@testing-library/react";
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
    expect(
      screen.getByText(
        /Selected step · 20s · receipt → start 40s · receipt → result 1m 0s/,
      ),
    ).toBeInTheDocument();
  });
  it("shows receipt-to-publication and selected authoritative stage timings with trace links", () => {
    usePipelineGraph.mockReturnValue({
      data: run,
      isLoading: false,
      error: null,
    });
    render(<PipelinePanel runId="run" />);
    expect(screen.getByText(/receipt → available 1m 0s/)).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "Open trace" }).getAttribute("href"),
    ).toContain("/explore?left=");
    fireEvent.click(screen.getByRole("button", { name: "Select saved stage" }));
    expect(
      screen.getByText(
        /Selected step · 10s · receipt → start 20s · receipt → result 30s/,
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Receipt → queued 10s · receipt → first start N\/A/),
    ).toBeInTheDocument();
    expect(screen.getByText(/1 activity saved/)).toBeInTheDocument();
  });
  it("keeps missing readiness unknown", () => {
    usePipelineGraph.mockReturnValue({
      data: { ...run, available_at: null, ended_at: null, tasks: [] },
      isLoading: false,
      error: null,
    });
    render(<PipelinePanel runId="run" />);
    expect(screen.getByText(/receipt → available N\/A/)).toBeInTheDocument();
  });
});
