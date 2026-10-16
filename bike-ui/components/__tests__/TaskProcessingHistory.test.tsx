import { render, screen, within } from "@testing-library/react";
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
    ...overrides,
  };
}

describe("TaskProcessingHistory", () => {
  it("shows recorded execution attempts without a reconstructed pipeline", () => {
    render(<TaskProcessingHistory detail={detail()} />);
    expect(screen.getAllByText("1m 0s")).toHaveLength(2);
    expect(screen.getByText("Provider timeout")).toBeVisible();
    expect(screen.getByText("Span: span-first")).toBeVisible();
    expect(screen.getByText("Span: span-second")).toBeVisible();
    expect(
      screen.queryByText("View complete pipeline"),
    ).not.toBeInTheDocument();
  });

  it("shows when execution attempts are absent", () => {
    render(<TaskProcessingHistory detail={detail({ attempt_history: [] })} />);
    expect(
      screen.getByText("No execution attempts have been recorded."),
    ).toBeVisible();
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
      render(
        <TaskProcessingHistory
          detail={detail({
            status: "processing",
            completed_at: null,
            attempt_history: [attempt],
          })}
        />,
      );
      const row = screen.getByText("running").closest("tr")!;
      expect(within(row).getByText("2m 0s")).toBeVisible();
      expect(screen.queryByText(/^Trace:/)).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });
});
