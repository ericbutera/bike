import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ActivityTaskMenu, UserTaskMenu } from "../admin/AdminTaskMenu";
import {
  acceptedTask,
  adminActivity,
  adminUser,
} from "@/tests/fixtures/adminTasks";
import { initializeClientConfig } from "@/lib/config";

const mocks = vi.hoisted(() => ({
  submit: vi.fn(),
  push: vi.fn(),
  pending: false,
}));
vi.mock("@/lib/adminTasks", () => ({
  useAdminTask: () => ({ submit: mocks.submit, isPending: mocks.pending }),
}));
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: mocks.push }) }));

beforeEach(() => {
  vi.clearAllMocks();
  mocks.pending = false;
  mocks.submit.mockReset().mockResolvedValue(acceptedTask());
  initializeClientConfig({ TEMPORAL_UI_URL: "https://temporal.example.com" });
  vi.spyOn(HTMLDialogElement.prototype, "showModal").mockImplementation(
    function (this: HTMLDialogElement) {
      this.setAttribute("open", "");
    },
  );
});
afterEach(() => {
  vi.restoreAllMocks();
  initializeClientConfig({});
});

async function choose(label: string) {
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: /Tasks for/ }));
  await user.click(screen.getByRole("menuitem", { name: label }));
  return user;
}

describe("activity task menu", () => {
  it.each([
    ["Reprocess activity", "reprocess"],
    ["Rebuild fitness / freshness", "rebuild_fitness"],
    ["Rebuild activity segments", "rebuild_segments"],
    ["Prepare activity heatmap", "rebuild_heatmap"],
  ])("submits %s from its activity row", async (label, task) => {
    render(<ActivityTaskMenu activity={adminActivity()} />);
    const user = await choose(label);
    if (task === "rebuild_fitness")
      expect(
        screen.getByText("Applies to all activities for user #7."),
      ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Run task" }));
    expect(mocks.submit).toHaveBeenCalledWith({ task });
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Task accepted",
    );
    expect(
      screen.getByRole("link", { name: "View job in Temporal" }),
    ).toHaveAttribute(
      "href",
      expect.stringContaining("/namespaces/bike/workflows/"),
    );
    expect(
      screen.getByRole("link", { name: "View integration events" }),
    ).toHaveAttribute(
      "href",
      "/admin/integrations?provider=&user_id=7&activity_id=42",
    );
  });

  it("resyncs native Strava activities and disables retained-file reprocessing", async () => {
    render(
      <ActivityTaskMenu activity={adminActivity({ source: "strava_sync" })} />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /Tasks for/ }));
    expect(
      screen.getByRole("menuitem", { name: /Reprocess activity/ }),
    ).toBeDisabled();
    await user.click(
      screen.getByRole("menuitem", { name: "Resync activity from Strava" }),
    );
    await user.click(screen.getByRole("button", { name: "Run task" }));
    expect(mocks.submit).toHaveBeenCalledWith({ task: "strava_resync" });
  });

  it("keeps import details accessible and explains unsupported source actions", async () => {
    render(<ActivityTaskMenu activity={adminActivity()} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /Tasks for/ }));
    expect(
      screen.getByRole("menuitem", { name: /Resync activity from Strava/ }),
    ).toBeDisabled();
    await user.click(
      screen.getByRole("menuitem", { name: "View import details" }),
    );
    expect(mocks.push).toHaveBeenCalledWith("/admin/imports/17");
  });
});

describe("user task menu", () => {
  it.each([
    ["Rebuild fitness / freshness", "rebuild_fitness"],
    ["Rebuild user segments", "rebuild_segments"],
    ["Backfill XC training", "backfill_xc_training"],
    ["Sync Strava", "strava_sync"],
  ])("submits %s without opening the account editor", async (label, task) => {
    const editUser = vi.fn();
    render(
      <div onClick={editUser}>
        <UserTaskMenu user={adminUser()} />
      </div>,
    );
    const user = await choose(label);
    await user.click(screen.getByRole("button", { name: "Run task" }));
    expect(mocks.submit).toHaveBeenCalledWith({ task });
    expect(editUser).not.toHaveBeenCalled();
  });

  it("offers the archive FIT filter for user reprocessing", async () => {
    render(<UserTaskMenu user={adminUser()} />);
    const user = await choose("Reprocess user imports");
    await user.click(
      screen.getByRole("checkbox", { name: "Archive FIT imports only" }),
    );
    await user.click(screen.getByRole("button", { name: "Run task" }));
    expect(mocks.submit).toHaveBeenCalledWith({
      task: "reprocess_imports",
      archive_fits_only: true,
    });
  });

  it("includes the selected UTC start and end days in a Strava backfill", async () => {
    render(<UserTaskMenu user={adminUser()} />);
    const user = await choose("Backfill Strava");
    fireEvent.change(screen.getByLabelText("Start date"), {
      target: { value: "2026-10-01" },
    });
    fireEvent.change(screen.getByLabelText("End date"), {
      target: { value: "2026-10-02" },
    });
    await user.click(screen.getByRole("button", { name: "Run task" }));
    expect(mocks.submit).toHaveBeenCalledWith({
      task: "strava_backfill",
      after: "2026-10-01T00:00:00.000Z",
      before: "2026-10-03T00:00:00.000Z",
    });
  });

  it("keeps failed submissions retryable without claiming completion", async () => {
    mocks.submit.mockRejectedValueOnce(new Error("Temporal unavailable"));
    render(<UserTaskMenu user={adminUser()} />);
    const user = await choose("Sync Strava");
    await user.click(screen.getByRole("button", { name: "Run task" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Temporal unavailable",
    );
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Run task" }));
    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("Task accepted"),
    );
  });

  it("blocks duplicate clicks while waiting for acknowledgement", async () => {
    mocks.pending = true;
    render(<UserTaskMenu user={adminUser()} />);
    await choose("Sync Strava");
    expect(screen.getByRole("button", { name: "Submitting…" })).toBeDisabled();
  });
});
