import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  useAdminTask,
  useAdminTasks,
  useActivityPipelines,
  usePipelineGraph,
} from "./queries";

let client: QueryClient;
const fetchMock = vi.fn();

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

beforeEach(() => {
  client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: Infinity } },
  });
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  client.clear();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("admin task queries", () => {
  it("does not fetch an unselected pipeline and sends work and output pagination to the API", async () => {
    const { unmount } = renderHook(() => usePipelineGraph(null), { wrapper });
    expect(fetchMock).not.toHaveBeenCalled();
    unmount();
    fetchMock.mockResolvedValue(
      Response.json({
        run_id: "run",
        ended_at: "2026-10-10T12:00:00Z",
        tasks: [],
        outputs: [],
      }),
    );
    const { result } = renderHook(
      () =>
        usePipelineGraph("run", 42, {
          work_task: 43,
          after_work: 7,
          output_offset: 100,
        }),
      { wrapper },
    );
    await waitFor(() => expect(result.current.data?.run_id).toBe("run"));
    const url = new URL((fetchMock.mock.calls[0][0] as Request).url);
    expect(url.pathname).toMatch(/\/admin\/tasks\/pipelines\/run$/);
    expect(Object.fromEntries(url.searchParams)).toEqual({
      after_task: "42",
      work_task: "43",
      after_work: "7",
      output_offset: "100",
    });
  });
  it("requests activity lineage from its direct activity endpoint and run cursor", async () => {
    fetchMock.mockResolvedValue(Response.json({ run_ids: ["next"] }));
    const { result } = renderHook(() => useActivityPipelines(7, "previous"), {
      wrapper,
    });
    await waitFor(() => expect(result.current.data?.run_ids).toEqual(["next"]));
    const url = new URL((fetchMock.mock.calls[0][0] as Request).url);
    expect(url.pathname).toMatch(/\/admin\/tasks\/activities\/7\/pipelines$/);
    expect(url.searchParams.get("after_run")).toBe("previous");
  });
  it("sends the error-log correlation ID to the API and returns its matching tasks", async () => {
    fetchMock.mockResolvedValue(
      Response.json({
        data: [{ id: 42, task_type: "prepare_heatmap", status: "pending" }],
        metadata: { total: 1, per_page: 20 },
      }),
    );
    const { result } = renderHook(
      () => useAdminTasks({ correlationId: "request-123" }),
      { wrapper },
    );
    await waitFor(() => expect(result.current.data?.[0].id).toBe(42));
    const request = fetchMock.mock.calls[0][0] as Request;
    expect(new URL(request.url).searchParams.get("correlation_id")).toBe(
      "request-123",
    );
    expect(result.current.metadata?.total).toBe(1);
  });

  it("refreshes an active task and stops polling when it completes", async () => {
    vi.useFakeTimers();
    fetchMock
      .mockResolvedValueOnce(
        Response.json({
          id: 42,
          status: "processing",
          attempt_history: [],
          pipelines: [],
        }),
      )
      .mockResolvedValue(
        Response.json({
          id: 42,
          status: "completed",
          attempt_history: [],
          pipelines: [],
        }),
      );
    const { result, unmount } = renderHook(() => useAdminTask(42), { wrapper });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(50);
    });
    expect(result.current.data?.status).toBe("processing");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5050);
    });
    expect(result.current.data?.status).toBe("completed");
    expect(fetchMock).toHaveBeenCalledTimes(2);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15000);
    });
    expect(fetchMock).toHaveBeenCalledTimes(2);
    unmount();
  });
});
