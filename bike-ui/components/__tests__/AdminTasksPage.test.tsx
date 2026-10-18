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
  expect(within(table).getByText("Completed")).toHaveClass("badge-success");
  expect(
    within(table)
      .getAllByRole("columnheader")
      .slice(-3)
      .map((cell) => cell.textContent),
  ).toEqual(["Started", "Finished", "Parameters"]);
  const taskCells = within(within(table).getAllByRole("row")[1]).getAllByRole(
    "cell",
  );
  expect(
    within(taskCells.at(-1)!).getByRole("button", { name: "Details" }),
  ).toBeVisible();
  expect(
    screen.queryByRole("heading", { name: "Background tasks" }),
  ).not.toBeInTheDocument();
  expect(
    within(table).getByText("rebuild fitness freshness"),
  ).toBeInTheDocument();
  expect(mocks.input).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Details" }));
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
    screen.queryByRole("button", { name: "Previous page" }),
  ).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "2" }));
  expect(mocks.history).toHaveBeenLastCalledWith(
    undefined,
    "next",
    undefined,
    true,
  );
  expect(
    screen.queryByRole("button", { name: "Next page" }),
  ).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "1" }));
  expect(mocks.history).toHaveBeenLastCalledWith(
    undefined,
    undefined,
    undefined,
    true,
  );
  await userEvent.click(screen.getByRole("button", { name: "2" }));
  fireEvent.change(screen.getByLabelText("Task status"), {
    target: { value: "Failed" },
  });
  expect(mocks.history).toHaveBeenLastCalledWith(
    "Failed",
    undefined,
    undefined,
    true,
  );
});

it("refreshes recent runs from the last control in the filter toolbar", async () => {
  render(<AdminTasksPage />);
  const refresh = screen.getByRole("button", { name: "Refresh" });
  const toolbar = screen.getByLabelText("Task status").closest("form");
  expect(toolbar?.lastElementChild).toBe(refresh);
  expect(refresh).not.toHaveTextContent("Refresh");
  expect(refresh.querySelector('svg[data-icon="arrows-rotate"]')).toBeVisible();
  await userEvent.click(refresh);
  expect(mocks.refresh).toHaveBeenCalledOnce();
  await userEvent.click(screen.getByRole("button", { name: "2" }));
  await userEvent.click(screen.getByRole("button", { name: "Refresh" }));
  expect(mocks.history).toHaveBeenLastCalledWith(
    undefined,
    undefined,
    undefined,
    true,
  );
});

it.each([
  [{ isLoading: true }, "Loading..."],
  [{ data: { executions: [] } }, "No tasks match these filters."],
  [
    { error: new Error("unavailable") },
    "Task history is unavailable. Try again or open Temporal.",
  ],
])("shows the history request state", (response, message) => {
  mocks.history.mockReturnValue(response);
  render(<AdminTasksPage />);
  expect(screen.getByText(message)).toBeInTheDocument();
});

it("filters by user before paging, resets cursors, and uses the shared toolbar", async () => {
  render(<AdminTasksPage />);
  const input = screen.getByLabelText("Task user ID");
  const table = screen.getByRole("table", { name: "Task history" });
  expect(input.closest("form")).toHaveClass("justify-end");
  expect(input.closest(".card")).toBe(table.closest(".card"));
  await userEvent.click(screen.getByRole("button", { name: "2" }));
  fireEvent.change(input, { target: { value: "7" } });
  expect(mocks.history).toHaveBeenLastCalledWith(undefined, undefined, 7, true);
  expect(screen.getByRole("button", { name: "1" })).toHaveAttribute(
    "aria-current",
    "page",
  );
  await userEvent.click(screen.getByRole("button", { name: "2" }));
  expect(mocks.history).toHaveBeenLastCalledWith(undefined, "next", 7, true);
  await userEvent.click(screen.getByRole("button", { name: "Clear" }));
  expect(input).toHaveValue(null);
  expect(mocks.history).toHaveBeenLastCalledWith(
    undefined,
    undefined,
    undefined,
    true,
  );
  expect(
    screen.getByRole("button", { name: "1" }).compareDocumentPosition(table) &
      Node.DOCUMENT_POSITION_PRECEDING,
  ).toBeTruthy();
});

it("does not fetch an invalid user filter", () => {
  render(<AdminTasksPage />);
  fireEvent.change(screen.getByLabelText("Task user ID"), {
    target: { value: "1.5" },
  });
  expect(mocks.history).toHaveBeenLastCalledWith(
    undefined,
    undefined,
    1.5,
    false,
  );
  expect(screen.getByRole("alert")).toHaveTextContent(
    "User ID must be a positive integer.",
  );
  expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
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
    await userEvent.click(screen.getByRole("button", { name: "Details" }));
    expect(
      within(screen.getByRole("dialog")).getByText(message),
    ).toBeInTheDocument();
  },
);
