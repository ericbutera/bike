import { renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  useActivityImportHistory,
  useActivityImportReplayPlan,
  useReplayActivityImport,
} from "./queries";

const api = vi.hoisted(() => ({
  query: vi.fn(),
  mutation: vi.fn(),
  replay: vi.fn(),
}));
vi.mock("./api", () => ({
  $api: {},
  $typedApi: { useQuery: api.query, useMutation: api.mutation },
}));

beforeEach(() => {
  vi.clearAllMocks();
  api.query.mockReturnValue({ data: { total: 1 } });
  api.replay.mockResolvedValue({ attempt_id: 12 });
  api.mutation.mockReturnValue({ mutateAsync: api.replay });
});

describe("import query hooks", () => {
  it("polls import history with the selected archive and source filters", () => {
    const query = { page: 2, source: "manual_upload", archive_job_id: 9 };
    const { result } = renderHook(() => useActivityImportHistory(query));
    expect(api.query).toHaveBeenCalledWith(
      "get",
      "/activity-imports/history",
      { params: { query } },
      { refetchInterval: 3000 },
    );
    expect(result.current.data).toEqual({ total: 1 });
  });

  it("previews the selected import and stage only when enabled", () => {
    renderHook(() => useActivityImportReplayPlan(7, "activity_saved", true));
    expect(api.query).toHaveBeenCalledWith(
      "get",
      "/activity-imports/{id}/replay",
      { params: { path: { id: 7 }, query: { stage: "activity_saved" } } },
      { enabled: true },
    );
  });

  it("queues the reviewed replay plan and refreshes its history and trace", async () => {
    const client = new QueryClient();
    const invalidate = vi.spyOn(client, "invalidateQueries");
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
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
      "/activity-imports/{id}/trace",
      "/activity-imports/history",
      "/activity-imports/{id}/replay",
    ]) {
      expect(invalidate).toHaveBeenCalledWith({ queryKey: ["get", path] });
    }
    client.clear();
  });
});
