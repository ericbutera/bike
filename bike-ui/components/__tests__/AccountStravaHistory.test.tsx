import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AccountPage from "../../app/account/page";

const mocks = vi.hoisted(() => ({
  connection: vi.fn(),
  events: vi.fn(),
  refetch: vi.fn(),
  queue: vi.fn(),
}));
vi.mock("next/navigation", () => ({
  useRouter: () => ({ replace: vi.fn() }),
  useSearchParams: () => new URLSearchParams(),
}));
vi.mock("react-hot-toast", () => ({
  default: { success: vi.fn(), error: vi.fn() },
}));
vi.mock("../Layout", () => ({
  default: ({ children }: { children: ReactNode }) => children,
}));
vi.mock("../RequireAuth", () => ({
  default: ({ children }: { children: ReactNode }) => children,
}));
vi.mock("@/lib/queries", () => ({
  useStravaConnection: mocks.connection,
  useStravaIntegrationEvents: mocks.events,
  useQueueStravaSync: () => ({ queueAsync: mocks.queue, isPending: false }),
  useUserPreferences: () => ({
    data: { unit_system: "metric" },
    isLoading: false,
  }),
  useUpdateUserPreferences: () => ({ isPending: false }),
  useDisconnectStrava: () => ({ isPending: false }),
  useStartStravaConnect: () => ({ isPending: false }),
  useActivity: () => ({ data: null, error: null }),
}));
const connection = {
  configured: true,
  connected: true,
  scopes: [],
  athlete_name: "Rider",
  last_sync_status: "succeeded",
  last_sync_finished_at: "2026-10-12T12:00:00Z",
  last_sync_imported_count: 0,
  last_sync_duplicate_count: 0,
  last_sync_failed_count: 0,
};
beforeEach(() => {
  vi.clearAllMocks();
  mocks.queue.mockResolvedValue(undefined);
  mocks.refetch.mockResolvedValue(undefined);
  mocks.connection.mockReturnValue({ data: connection, isLoading: false });
  mocks.events.mockReturnValue({
    data: [],
    isLoading: false,
    error: null,
    refetch: mocks.refetch,
  });
});
describe("account Strava sync history", () => {
  it("keeps Sync now available and queues the requested sync", async () => {
    render(<AccountPage />);
    fireEvent.click(screen.getByRole("button", { name: "Sync now" }));
    await waitFor(() => expect(mocks.queue).toHaveBeenCalledOnce());
  });
  it("refreshes history on the terminal status transition even after active polling stops", () => {
    mocks.connection.mockReturnValue({
      data: { ...connection, last_sync_status: "running" },
      isLoading: false,
    });
    const { rerender } = render(<AccountPage />);
    expect(mocks.events).toHaveBeenLastCalledWith({ refetchIntervalMs: 5000 });
    mocks.refetch.mockClear();
    mocks.connection.mockReturnValue({
      data: { ...connection, last_sync_finished_at: "2026-10-13T12:00:00Z" },
      isLoading: false,
    });
    mocks.events.mockReturnValue({
      data: [
        {
          id: 7,
          provider: "strava",
          event_type: "strava.event.accepted",
          level: "info",
          message: "Strava Sync requested.",
          payload: {
            operation: "sync",
            temporal_workflow_id: "private",
            temporal_namespace: "bike",
          },
          created_at: "2026-10-13T12:00:00Z",
        },
      ],
      isLoading: false,
      error: null,
      refetch: mocks.refetch,
    });
    rerender(<AccountPage />);
    expect(mocks.events).toHaveBeenLastCalledWith({ refetchIntervalMs: false });
    expect(mocks.refetch).toHaveBeenCalledOnce();
    expect(screen.getByText("Up to date")).toBeInTheDocument();
    expect(screen.getByText("Strava Sync requested.")).toBeInTheDocument();
    expect(screen.queryByText(/private/)).not.toBeInTheDocument();
  });
});
