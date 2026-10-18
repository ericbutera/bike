import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import AdminTasksPage from "../../app/admin/tasks/page";
import { initializeClientConfig } from "../../lib/config";
import { taskExecution } from "@/tests/fixtures/adminTasks";

const mocks = vi.hoisted(() => ({
  history: vi.fn(),
  input: vi.fn(),
  refresh: vi.fn(),
}));
vi.mock("@/lib/adminTaskHistory", () => ({
  useTaskHistory: mocks.history,
  useTaskInput: mocks.input,
}));

vi.mock("../admin/LocalAdminLayout", () => ({
  LocalAdminLayout: ({ children }: { children: ReactNode }) => children,
}));

beforeEach(() => {
  vi.clearAllMocks();
  initializeClientConfig({ TEMPORAL_UI_URL: "https://temporal.example.com" });
  mocks.history.mockImplementation((_status?: string, token?: string) => ({
    data: {
      executions: [taskExecution()],
      next_page_token: token ? null : "next",
    },
    refetch: mocks.refresh,
  }));
  mocks.input.mockReturnValue({
    data: { parameters: [{ user_id: 7, after_id: 12 }] },
  });
  vi.spyOn(HTMLDialogElement.prototype, "showModal").mockImplementation(
    function (this: HTMLDialogElement) {
      this.setAttribute("open", "");
    },
  );
});

afterEach(() => {
  initializeClientConfig({});
  vi.restoreAllMocks();
});

it("shows prior executions and loads the selected run's parameters on demand", async () => {
  render(<AdminTasksPage />);
  const table = screen.getByRole("table", { name: "Task history" });
  expect(within(table).getByText("Completed")).toBeInTheDocument();
  expect(
    within(table).getByText("rebuild fitness freshness"),
  ).toBeInTheDocument();
  expect(mocks.input).not.toHaveBeenCalled();
  await userEvent.click(
    screen.getByRole("button", { name: "View parameters" }),
  );
  expect(mocks.input).toHaveBeenCalledWith(taskExecution().job);
  expect(screen.getByRole("dialog")).toHaveTextContent('"user_id": 7');
  expect(screen.getByRole("dialog")).toHaveTextContent('"after_id": 12');
  expect(
    screen.getByRole("link", { name: "View job in Temporal" }),
  ).toHaveAttribute(
    "href",
    "https://temporal.example.com/namespaces/bike/workflows/bike-fitness-1/run-1",
  );
  await userEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

it("pages older and newer results and resets paging when the status changes", async () => {
  render(<AdminTasksPage />);
  expect(
    screen.queryByRole("button", { name: "Newer" }),
  ).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Older" }));
  expect(mocks.history).toHaveBeenLastCalledWith(undefined, "next");
  expect(
    screen.queryByRole("button", { name: "Older" }),
  ).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Newer" }));
  expect(mocks.history).toHaveBeenLastCalledWith(undefined, undefined);
  await userEvent.click(screen.getByRole("button", { name: "Older" }));
  fireEvent.change(screen.getByLabelText("Task status"), {
    target: { value: "Failed" },
  });
  expect(mocks.history).toHaveBeenLastCalledWith("Failed", undefined);
});

it("refreshes recent runs and keeps native execution controls available", async () => {
  render(<AdminTasksPage />);
  await userEvent.click(screen.getByRole("button", { name: "Refresh" }));
  expect(mocks.refresh).toHaveBeenCalledOnce();
  await userEvent.click(screen.getByRole("button", { name: "Older" }));
  await userEvent.click(screen.getByRole("button", { name: "Refresh" }));
  expect(mocks.history).toHaveBeenLastCalledWith(undefined, undefined);
  expect(
    screen.getByRole("link", { name: "Open Temporal workflows" }),
  ).toHaveAttribute(
    "href",
    "https://temporal.example.com/namespaces/bike/workflows",
  );
});

it.each([
  [{ isLoading: true }, "Loading..."],
  [{ data: { executions: [] } }, "No tasks match this status."],
  [
    { error: new Error("unavailable") },
    "Task history is unavailable. Try again or open Temporal.",
  ],
])("shows the history request state", (response, message) => {
  mocks.history.mockReturnValue(response);
  render(<AdminTasksPage />);
  expect(screen.getByText(message)).toBeInTheDocument();
});

it.each([
  [{ isLoading: true }, "Loading parameters..."],
  [
    { error: new Error("expired") },
    "Parameters are unavailable. Try again or open Temporal.",
  ],
])(
  "shows parameter loading or unavailable retained history",
  async (response, message) => {
    mocks.input.mockReturnValue(response);
    render(<AdminTasksPage />);
    await userEvent.click(
      screen.getByRole("button", { name: "View parameters" }),
    );
    expect(
      within(screen.getByRole("dialog")).getByText(message),
    ).toBeInTheDocument();
  },
);
