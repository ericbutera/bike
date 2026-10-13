import {
  QueryClient,
  QueryClientProvider,
  focusManager,
  onlineManager,
} from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  useActivityImportTrace,
  useAdminActivityImportTrace,
  useActivityImportHistory,
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
  fetchMock.mockReset().mockImplementation(async () =>
    Response.json({
      import: { id: 7 },
      trace: { import: { id: 7 } },
      items: [],
    }),
  );
  vi.stubGlobal("fetch", fetchMock);
  vi.useFakeTimers();
});
afterEach(() => {
  client.clear();
  focusManager.setFocused(undefined);
  onlineManager.setOnline(true);
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
describe("on-demand admin import queries", () => {
  it.each([
    ["import trace", () => useActivityImportTrace(7)],
    ["activity import trace", () => useAdminActivityImportTrace(7)],
    ["import history", () => useActivityImportHistory({ page: 1 })],
  ] as const)(
    "does not poll or automatically refresh %s",
    async (_name, hook) => {
      const { result } = renderHook(() => hook(), { wrapper });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1);
      });
      expect(result.current.isSuccess).toBe(true);
      expect(fetchMock).toHaveBeenCalledOnce();
      const firstData = result.current.data;
      await act(async () => {
        focusManager.setFocused(false);
        onlineManager.setOnline(false);
        focusManager.setFocused(true);
        onlineManager.setOnline(true);
        await vi.advanceTimersByTimeAsync(30000);
      });
      expect(fetchMock).toHaveBeenCalledOnce();
      expect(result.current.data).toBe(firstData);
      await act(async () => {
        await result.current.refetch();
        await vi.advanceTimersByTimeAsync(1);
      });
      expect(fetchMock).toHaveBeenCalledTimes(2);
      expect(result.current.isSuccess).toBe(true);
    },
  );
  it("does not request an unselected or disabled trace", async () => {
    renderHook(
      () => ({
        missing: useActivityImportTrace(null),
        disabled: useActivityImportTrace(7, { enabled: false }),
      }),
      { wrapper },
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(30000);
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
