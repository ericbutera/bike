import { act, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { acceptedTask, taskTarget } from "@/tests/fixtures/adminTasks";
import { useAdminTask } from "./adminTasks";

const api = vi.hoisted(() => ({ mutation: vi.fn(), submit: vi.fn() }));
vi.mock("./api", () => ({ $typedApi: { useMutation: api.mutation } }));
let client: QueryClient;
function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

beforeEach(() => {
  client = new QueryClient();
  vi.clearAllMocks();
  api.mutation.mockReturnValue({ mutateAsync: api.submit, isPending: false });
  api.submit.mockReset().mockResolvedValue(acceptedTask());
});
afterEach(() => client.clear());

describe("admin task submission", () => {
  it.each(["activity", "user"] as const)(
    "retries a lost %s acknowledgement with the original command ID",
    async (kind) => {
      api.submit.mockRejectedValueOnce(new TypeError("Lost reply"));
      const { result, rerender } = renderHook(
        () => useAdminTask(taskTarget({ kind })),
        { wrapper },
      );
      await act(async () => {
        await expect(
          result.current.submit({ task: "rebuild_fitness" }),
        ).rejects.toThrow("Lost reply");
      });
      rerender();
      await act(async () => {
        await result.current.submit({ task: "rebuild_fitness" });
      });
      expect(api.submit.mock.calls[1][0]).toEqual(api.submit.mock.calls[0][0]);
      expect(api.submit.mock.calls[0][0]).toEqual({
        params: { path: { id: 7 } },
        body: { task: "rebuild_fitness", command_id: expect.any(String) },
      });
      await act(async () => {
        await result.current.submit({ task: "rebuild_fitness" });
      });
      expect(api.submit.mock.calls[2][0].body.command_id).not.toBe(
        api.submit.mock.calls[0][0].body.command_id,
      );
    },
  );

  it("keeps different parameters and targets separate while a request is uncertain", async () => {
    api.submit.mockRejectedValue(new TypeError("Lost reply"));
    const { result, rerender } = renderHook(
      ({ id }) => useAdminTask(taskTarget({ id })),
      { wrapper, initialProps: { id: 7 } },
    );
    for (const [id, archiveOnly] of [
      [7, false],
      [8, false],
      [7, true],
      [7, false],
    ] as const) {
      rerender({ id });
      await act(async () => {
        await expect(
          result.current.submit({
            task: "reprocess_imports",
            archive_fits_only: archiveOnly,
          }),
        ).rejects.toThrow();
      });
    }
    const bodies = api.submit.mock.calls.map(([request]) => request.body);
    expect(bodies[0]).toEqual(bodies[3]);
    expect(new Set(bodies.map((body) => body.command_id)).size).toBe(3);
  });

  it("refreshes the integration feed on both success and failure", async () => {
    const invalidate = vi.spyOn(client, "invalidateQueries");
    renderHook(() => useAdminTask(taskTarget()), { wrapper });
    for (const [method, path, options] of api.mutation.mock.calls) {
      expect(method).toBe("post");
      expect(path).toMatch(/^\/admin\/(activities|users)\/\{id\}\/tasks$/);
      await options.onSettled();
    }
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["get", "/admin/integration-events"],
    });
  });
});
