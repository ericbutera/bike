import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ActivityImportTrace } from "../../lib/queries";
import ActivityImportTracePanel from "../activity-detail/ActivityImportTracePanel";

const mocks = vi.hoisted(() => ({ plan: vi.fn(), replay: vi.fn() }));
vi.mock("../../lib/queries", () => ({
  useActivityImportReplayPlan: mocks.plan,
  useReplayActivityImport: () => ({ replay: mocks.replay, isPending: false }),
}));
vi.mock("react-hot-toast", () => ({
  default: { success: vi.fn(), error: vi.fn() },
}));

function trace(): ActivityImportTrace {
  const nodes = [
    {
      id: "raw_stored",
      label: "Raw stored",
      stage: "raw_stored",
      status: "completed",
      summary: ["GPX · 4096 bytes"],
    },
    {
      id: "activity_parsed",
      label: "Activity parsed",
      stage: "activity_parsed",
      status: "completed",
      summary: ["Parsed 7 records", "7 chart samples · 0 laps"],
    },
    {
      id: "activity_saved",
      label: "Activity saved",
      stage: "activity_saved",
      status: "completed",
      summary: ["Saved activity #42"],
    },
  ];
  return {
    import: {
      id: 17,
      import_version: 2,
      source: "manual_upload",
      status: "processed",
      original_filename: "ride.gpx",
      processing_stage: "complete",
      size_bytes: 4096,
      created_at: "2026-10-08T12:00:00Z",
    },
    graph: { nodes, edges: [], mermaid: "flowchart LR" },
    nodes,
    events: [
      {
        id: 99,
        event_type: "stage_completed",
        level: "info",
        message: "Verbose event message",
        created_at: "2026-10-08T12:00:00Z",
      },
    ],
    attempts: [
      {
        id: 1,
        status: "completed",
        requested_stage: "raw_stored",
        start_stage: "raw_stored",
        source: {
          filename: "ride.gpx",
          format: "gpx",
          quality: "gpx_original",
        },
        created_at: "2026-10-08T12:00:00Z",
        nodes,
      },
    ],
  };
}

beforeEach(() => {
  mocks.plan.mockReset().mockImplementation((_id, stage) => ({
    data: {
      requested_stage: stage,
      start_stage: stage,
      reused_attempt_id: 1,
    },
    isLoading: false,
    error: null,
  }));
  mocks.replay.mockReset().mockResolvedValue({ attempt_id: 2 });
});

describe("Processing stages and replay", () => {
  it("shows compact stage summaries and omits the event log", () => {
    render(
      <ActivityImportTracePanel
        trace={trace()}
        isLoading={false}
        error={null}
      />,
    );
    expect(screen.getByText("Parsed 7 records")).toBeInTheDocument();
    expect(
      within(
        screen.getByRole("button", { name: "Raw stored completed" }),
      ).getByText("GPX · 4096 bytes"),
    ).toBeInTheDocument();
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
    expect(screen.queryByText("Verbose event message")).not.toBeInTheDocument();
  });

  it("selects a DAG node and queues a replay from that stage", async () => {
    const user = userEvent.setup();
    render(
      <ActivityImportTracePanel
        trace={trace()}
        isLoading={false}
        error={null}
        canReplay
      />,
    );
    await user.click(
      screen.getByRole("button", { name: "Activity saved completed" }),
    );
    expect(mocks.plan).toHaveBeenLastCalledWith(17, "activity_saved", true);
    await user.click(
      screen.getByRole("button", { name: "Replay from Activity saved" }),
    );
    expect(mocks.replay).toHaveBeenCalledWith(17, "activity_saved", {
      requested_stage: "activity_saved",
      start_stage: "activity_saved",
      reused_attempt_id: 1,
    });
    await waitFor(() =>
      expect(screen.getByText(/Queued attempt #2/)).toBeInTheDocument(),
    );
  });

  it("disables replay while an attempt is active", () => {
    const current = trace();
    current.attempts![0].status = "running";
    render(
      <ActivityImportTracePanel
        trace={current}
        isLoading={false}
        error={null}
        canReplay
      />,
    );
    expect(
      screen.getByRole("button", { name: "Replay from Raw stored" }),
    ).toBeDisabled();
    expect(mocks.plan).toHaveBeenLastCalledWith(17, "raw_stored", false);
  });

  it("explains a fallback to an earlier stage before queueing", async () => {
    const user = userEvent.setup();
    mocks.plan.mockReturnValue({
      data: {
        requested_stage: "activity_saved",
        start_stage: "raw_stored",
        reason: "Earlier results are missing or stale.",
      },
      isLoading: false,
      error: null,
    });
    render(
      <ActivityImportTracePanel
        trace={trace()}
        isLoading={false}
        error={null}
        canReplay
      />,
    );
    await user.click(
      screen.getByRole("button", { name: /Activity saved completed/ }),
    );
    expect(
      screen.getByText("Earlier results are missing or stale."),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Replay from Raw stored" }),
    ).toBeEnabled();
  });

  it("keeps older attempts and their errors distinct", async () => {
    const user = userEvent.setup();
    const current = trace();
    const prior = current.attempts![0];
    current.attempts = [
      {
        ...prior,
        id: 2,
        status: "failed",
        error: "Latest failure",
        nodes: prior.nodes.map((node) => ({ ...node, status: "failed" })),
      },
      prior,
    ];
    render(
      <ActivityImportTracePanel
        trace={current}
        isLoading={false}
        error={null}
      />,
    );
    expect(screen.getByText("Latest failure")).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Attempt #1 completed" }),
    );
    expect(screen.queryByText("Latest failure")).not.toBeInTheDocument();
    expect(screen.getByText("Parsed 7 records")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Raw stored completed" }),
    ).toHaveAttribute("aria-pressed", "true");
  });

  it("shows every selected stage fact and never mixes another attempt's timing", async () => {
    const user = userEvent.setup();
    const current = trace();
    current.attempts![0].nodes[1] = {
      ...current.nodes[1],
      started_at: "2026-10-08T12:00:05Z",
      completed_at: "2026-10-08T12:00:15Z",
    };
    render(
      <ActivityImportTracePanel
        trace={current}
        isLoading={false}
        error={null}
      />,
    );
    await user.click(
      screen.getByRole("button", { name: "Activity parsed completed" }),
    );
    const details = within(
      screen.getByRole("region", { name: "Selected stage" }),
    );
    expect(details.getByText("7 chart samples · 0 laps")).toBeInTheDocument();
    expect(details.getByText("10s")).toBeInTheDocument();
    expect(
      details.getByRole("link", { name: /Open activity processing/ }),
    ).toHaveAttribute("href", "/admin/imports/17");
  });

  it("names the actual replay stage when fallback stops at parsing", async () => {
    const user = userEvent.setup();
    mocks.plan.mockReturnValue({
      data: {
        requested_stage: "activity_saved",
        start_stage: "activity_parsed",
      },
      isLoading: false,
      error: null,
    });
    render(
      <ActivityImportTracePanel
        trace={trace()}
        isLoading={false}
        error={null}
        canReplay
      />,
    );
    await user.click(
      screen.getByRole("button", { name: "Activity saved completed" }),
    );
    await user.click(
      screen.getByRole("button", { name: "Replay from Activity parsed" }),
    );
    expect(mocks.replay).toHaveBeenCalledWith(17, "activity_saved", {
      requested_stage: "activity_saved",
      start_stage: "activity_parsed",
    });
  });
});
