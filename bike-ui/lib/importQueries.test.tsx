import { act, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { acceptedReprocessing } from "@/tests/fixtures/reprocessing";
import {
  useActivityImportHistory,
  useActivityImportReplayPlan,
  useReplayActivityImport,
  useReprocessActivityImport,
} from "./queries";

const api = vi.hoisted(() => ({
  query: vi.fn(),
  mutation: vi.fn(),
  replay: vi.fn(),
}));
vi.mock("./api", () => ({
  $api: { useQuery: api.query, useMutation: api.mutation },
  $typedApi: { useQuery: api.query, useMutation: api.mutation },
}));

let client: QueryClient;

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

beforeEach(() => {
  client = new QueryClient();
  vi.clearAllMocks();
  api.query.mockReturnValue({ data: { total: 1 } });
  api.replay.mockResolvedValue({ attempt_id: 12 });
  api.mutation.mockReturnValue({ mutateAsync: api.replay });
});

afterEach(() => client.clear());

describe("import query hooks", () => {
  it("loads import history on demand with the selected archive and source filters", () => {
    const query = { page: 2, source: "manual_upload", archive_job_id: 9 };
    const { result } = renderHook(() => useActivityImportHistory(query));
    expect(api.query).toHaveBeenCalledWith(
      "get",
      "/admin/activity-imports/history",
      { params: { query } },
      {
        refetchInterval: false,
        refetchOnWindowFocus: false,
        refetchOnReconnect: false,
        enabled: true,
      },
    );
    expect(result.current.data).toEqual({ total: 1 });
  });

  it("previews the selected import and stage only when enabled", () => {
    renderHook(() => useActivityImportReplayPlan(7, "activity_saved", true));
    expect(api.query).toHaveBeenCalledWith(
      "get",
      "/admin/activity-imports/{id}/replay",
      { params: { path: { id: 7 }, query: { stage: "activity_saved" } } },
      { enabled: true },
    );
  });

  it("queues the reviewed replay plan and refreshes its history and trace", async () => {
    const invalidate = vi.spyOn(client, "invalidateQueries");
    const { result } = renderHook(useReplayActivityImport, { wrapper });
    await expect(
      result.current.replay(7, "activity_saved", {
        requested_stage: "activity_saved",
        start_stage: "activity_parsed",
        reused_attempt_id: 11,
      }),
    ).resolves.toEqual({ attempt_id: 12 });
    expect(api.replay).toHaveBeenCalledWith({
      params: { path: { id: 7 } },
      body: {
        stage: "activity_saved",
        expected_start_stage: "activity_parsed",
        expected_reused_attempt_id: 11,
      },
    });
    for (const path of [
      "/admin/activity-imports/{id}/trace",
      "/admin/activity-imports/history",
      "/admin/activity-imports/{id}/replay",
    ]) {
      expect(invalidate).toHaveBeenCalledWith({ queryKey: ["get", path] });
    }
  });

  it("retries a lost acceptance reply with the original command after rerender", async () => {
    api.replay.mockResolvedValue(acceptedReprocessing({ activity_id: 7 }));
    api.replay.mockRejectedValueOnce(new TypeError("Lost acceptance reply"));
    const { result, rerender } = renderHook(useReprocessActivityImport, {
      wrapper,
    });
    await act(async () => {
      await expect(result.current.reprocessAsync(7)).rejects.toThrow(
        "Lost acceptance reply",
      );
    });
    rerender();
    await act(async () => {
      await result.current.reprocessAsync("7");
    });
    expect(api.replay.mock.calls[1][0]).toEqual(api.replay.mock.calls[0][0]);
    expect(api.replay.mock.calls[0][0].body).toEqual({
      activity_id: 7,
      command_id: expect.any(String),
    });
    await act(async () => {
      await result.current.reprocessAsync(7);
    });
    expect(api.replay.mock.calls[2][0].body.command_id).not.toBe(
      api.replay.mock.calls[0][0].body.command_id,
    );
  });

  it("keeps uncertain commands separate when the selected target changes", async () => {
    api.replay.mockRejectedValue(new TypeError("Lost acceptance reply"));
    const { result } = renderHook(useReprocessActivityImport, { wrapper });
    for (const activityId of [7, 8, 7]) {
      await act(async () => {
        await expect(
          result.current.reprocessAsync(activityId),
        ).rejects.toThrow();
      });
    }
    const bodies = api.replay.mock.calls.map(([request]) => request.body);
    expect(bodies[0]).toEqual(bodies[2]);
    expect(bodies[0].command_id).not.toBe(bodies[1].command_id);
    expect(bodies[1].activity_id).toBe(8);
  });
});
