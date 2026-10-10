import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { components } from "../../lib/openapi/react-query/api";
import TaskProcessingHistory from "../admin/TaskProcessingHistory";

type Detail = components["schemas"]["TaskDetailResponse"];

function detail(overrides: Partial<Detail> = {}): Detail {
  return {
    id: 42,
    task_type: "rebuild_fitness_freshness",
    status: "completed",
    attempts: 2,
    max_attempts: 3,
    created_at: "2026-10-10T12:00:00Z",
    updated_at: "2026-10-10T12:06:00Z",
    completed_at: "2026-10-10T12:06:00Z",
    attempt_history: [
      {
        attempt: 1,
        started_at: "2026-10-10T12:02:00Z",
        finished_at: "2026-10-10T12:03:00Z",
        heartbeat_at: "2026-10-10T12:02:30Z",
        progress_at: "2026-10-10T12:02:00Z",
        outcome: "retrying",
        error: "Provider timeout",
        trace_id: "trace-first",
        span_id: "span-first",
      },
      {
        attempt: 2,
        started_at: "2026-10-10T12:05:00Z",
        finished_at: "2026-10-10T12:06:00Z",
        heartbeat_at: "2026-10-10T12:05:30Z",
        progress_at: "2026-10-10T12:05:00Z",
        outcome: "completed",
        trace_id: "trace-first",
        span_id: "span-second",
      },
    ],
    pipelines: [
      {
        run_id: "pipeline-123",
        pipeline_started_at: "2026-10-10T12:00:00Z",
        accepted_at: "2026-10-10T12:01:00Z",
        entrypoint: "api",
        request_id: "request-123",
        parent_task_id: 7,
        available_at: null,
      },
    ],
    ...overrides,
  };
}

describe("TaskProcessingHistory", () => {
  it("keeps receipt-to-task timing separate from retries and unknown output readiness", async () => {
    const select = vi.fn();
    render(<TaskProcessingHistory detail={detail()} onSelectTask={select} />);
    expect(
      screen.getByText("Receipt to task start").nextElementSibling,
    ).toHaveTextContent("2m 0s");
    expect(
      screen.getByText("Receipt to task finish").nextElementSibling,
    ).toHaveTextContent("6m 0s");
    expect(
      screen.getByText("Receipt to available").nextElementSibling,
    ).toHaveTextContent("Not recorded");
    expect(screen.getAllByText("1m 0s")).toHaveLength(2);
    expect(screen.getByText("Provider timeout")).toBeVisible();
    expect(screen.getByText("Span: span-first")).toBeVisible();
    expect(screen.getByText("Span: span-second")).toBeVisible();
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Parent task #7" }));
    expect(select).toHaveBeenCalledWith(7);
  });

  it("shows missing historical evidence without fabricating a receipt timestamp", () => {
    render(
      <TaskProcessingHistory
        detail={detail({ pipelines: [], attempt_history: [] })}
        onSelectTask={vi.fn()}
      />,
    );
    expect(
      screen.getByText(
        "Original receipt and task lineage were not recorded for this task.",
      ),
    ).toBeVisible();
    expect(
      screen.getByText("No execution attempts have been recorded."),
    ).toBeVisible();
    expect(screen.queryByText("Receipt to available")).not.toBeInTheDocument();
  });

  it("shows active runtime while trace and parent evidence are absent", () => {
    vi.useFakeTimers();
    vi.setSystemTime("2026-10-10T12:07:00Z");
    try {
      const source = detail();
      const attempt = {
        ...source.attempt_history[1],
        outcome: "running",
        finished_at: null,
        error: null,
        trace_id: null,
        span_id: null,
      };
      const pipeline = {
        ...source.pipelines[0],
        request_id: null,
        trace_id: "pipeline-trace",
        parent_task_id: null,
      };
      render(
        <TaskProcessingHistory
          detail={detail({
            status: "processing",
            completed_at: null,
            attempt_history: [attempt],
            pipelines: [pipeline],
          })}
          onSelectTask={vi.fn()}
        />,
      );
      const row = screen.getByText("running").closest("tr")!;
      expect(within(row).getByText("2m 0s")).toBeVisible();
      expect(screen.getByText("Trace: pipeline-trace")).toBeVisible();
      expect(
        screen.getByText("Receipt to task finish").nextElementSibling,
      ).toHaveTextContent("Not recorded");
      expect(
        screen.queryByRole("button", { name: /Parent task/ }),
      ).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });
});
