import { renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { taskExecution } from "@/tests/fixtures/adminTasks";
import { initializeClientConfig } from "./config";
import { useTaskHistory, useTaskInput } from "./adminTaskHistory";

let client: QueryClient;
const fetchMock = vi.fn();
function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  initializeClientConfig({ API_URL: "https://bike.example/api" });
  vi.stubGlobal("fetch", fetchMock);
  fetchMock
    .mockReset()
    .mockResolvedValue(
      Response.json({ executions: [taskExecution()], next_page_token: "next" }),
    );
});
afterEach(() => {
  client.clear();
  vi.unstubAllGlobals();
  initializeClientConfig({});
});

it("passes status and opaque pagination to the authenticated history API", async () => {
  const { result } = renderHook(() => useTaskHistory("Completed", "0102"), {
    wrapper,
  });
  await waitFor(() => expect(result.current.isSuccess).toBe(true));
  const [request, init] = fetchMock.mock.calls[0];
  const url = new URL(request.url);
  expect(url.pathname).toBe("/api/admin/tasks");
  expect(url.searchParams.get("status")).toBe("Completed");
  expect(url.searchParams.get("page_token")).toBe("0102");
  expect(init.credentials).toBe("include");
  expect(result.current.data?.executions[0].status).toBe("Completed");
});

it("caches immutable inputs by exact workflow and run", async () => {
  fetchMock.mockImplementation(async () =>
    Response.json({ parameters: [{ user_id: 7 }] }),
  );
  const job = { ...taskExecution().job, job_id: "task/one" };
  const { result, rerender } = renderHook(
    ({ runId }) => useTaskInput({ ...job, run_id: runId }),
    { wrapper, initialProps: { runId: "run-1" } },
  );
  await waitFor(() => expect(result.current.isSuccess).toBe(true));
  expect(new URL(fetchMock.mock.calls[0][0].url).pathname).toBe(
    "/api/admin/tasks/task%2Fone/runs/run-1/input",
  );
  rerender({ runId: "run-2" });
  await waitFor(() => expect(result.current.isSuccess).toBe(true));
  expect(fetchMock).toHaveBeenCalledTimes(2);
  rerender({ runId: "run-1" });
  expect(result.current.data?.parameters).toEqual([{ user_id: 7 }]);
  expect(fetchMock).toHaveBeenCalledTimes(2);
});
