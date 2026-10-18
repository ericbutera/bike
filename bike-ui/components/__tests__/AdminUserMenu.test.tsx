import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { UserTaskMenu } from "../admin/AdminTaskMenu";
import { adminUser } from "@/tests/fixtures/adminTasks";

const mocks = vi.hoisted(() => ({
  details: vi.fn(),
  audit: vi.fn(),
  update: vi.fn(),
  disable: vi.fn(),
}));
vi.mock("../../lib/queries", () => ({
  useAdminUser: mocks.details,
  useAdminUserAuditEvents: mocks.audit,
  useUpdateAdminUser: () => ({ updateAsync: mocks.update, isPending: false }),
  useDisableAdminUser: () => ({
    disableAsync: mocks.disable,
    isPending: false,
  }),
}));
vi.mock("../../lib/adminTasks", () => ({ useAdminTask: () => ({}) }));
vi.mock("next/navigation", () => ({ useRouter: () => ({}) }));
vi.mock("react-hot-toast", () => ({ default: { success: vi.fn() } }));

beforeEach(() => {
  vi.clearAllMocks();
  mocks.details.mockReturnValue({
    data: { ...adminUser(), last_login_at: "2026-10-18T12:00:00Z" },
  });
  mocks.audit.mockReturnValue({
    data: {
      data: [
        {
          id: 1,
          event_type: "login_succeeded",
          ip: "127.0.0.1",
          user_agent: "Fixture browser",
          reason: null,
          occurred_at: "2026-10-18T12:00:00Z",
        },
      ],
      metadata: { page: 1, per_page: 20, total: 21 },
    },
  });
  mocks.update.mockResolvedValue(undefined);
  mocks.disable.mockResolvedValue(undefined);
  vi.spyOn(HTMLDialogElement.prototype, "showModal").mockImplementation(
    function (this: HTMLDialogElement) {
      this.setAttribute("open", "");
    },
  );
});
afterEach(() => vi.restoreAllMocks());

async function openMenu() {
  render(<UserTaskMenu user={adminUser()} />);
  await userEvent.click(screen.getByRole("button", { name: /Tasks for/ }));
}

it("separates viewing, editing and tasks without fetching details or audit history", async () => {
  await openMenu();
  expect(screen.getByText("Viewing").closest("details")).toHaveAttribute(
    "open",
  );
  expect(screen.getByText("Editing").closest("details")).not.toHaveAttribute(
    "open",
  );
  expect(
    screen.getByText("Task management").closest("details"),
  ).not.toHaveAttribute("open");
  expect(mocks.details).not.toHaveBeenCalled();
  expect(mocks.audit).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("menuitem", { name: "View details" }));
  expect(mocks.details).toHaveBeenCalledWith(7);
  expect(mocks.audit).not.toHaveBeenCalled();
  expect(screen.getByRole("dialog")).toHaveTextContent("Last login");
  expect(screen.getByRole("dialog")).toHaveTextContent("2026");
});

it("loads audit events only when selected and pages only that user's history", async () => {
  await openMenu();
  await userEvent.click(
    screen.getByRole("menuitem", { name: "View audit events" }),
  );
  expect(mocks.audit).toHaveBeenLastCalledWith(7, 1);
  expect(mocks.details).not.toHaveBeenCalled();
  const table = screen.getByRole("table", { name: "User audit events" });
  expect(within(table).getByText("login succeeded")).toBeVisible();
  expect(within(table).getAllByRole("columnheader").at(-1)).toHaveTextContent(
    "Occurred",
  );
  await userEvent.click(screen.getByRole("button", { name: "2" }));
  expect(mocks.audit).toHaveBeenLastCalledWith(7, 2);
  await userEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

it("edits an account through its Editing section without querying audit data", async () => {
  await openMenu();
  await userEvent.click(screen.getByText("Editing"));
  await userEvent.click(screen.getByRole("menuitem", { name: "Edit user" }));
  expect(mocks.details).not.toHaveBeenCalled();
  expect(mocks.audit).not.toHaveBeenCalled();
  await userEvent.clear(screen.getByLabelText("Name"));
  await userEvent.type(screen.getByLabelText("Name"), "Updated rider");
  await userEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(mocks.update).toHaveBeenCalledWith(7, {
    name: "Updated rider",
    is_admin: false,
  });
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

it("keeps account disablement in the editor and reports failed saves", async () => {
  mocks.update.mockRejectedValueOnce(new Error("Unavailable"));
  await openMenu();
  await userEvent.click(screen.getByText("Editing"));
  await userEvent.click(screen.getByRole("menuitem", { name: "Edit user" }));
  await userEvent.click(
    screen.getByRole("button", { name: "Disable Account" }),
  );
  expect(mocks.disable).toHaveBeenCalledWith(7, true);
  expect(
    screen.getByRole("button", { name: "Account Disabled" }),
  ).toBeDisabled();
  await userEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(screen.getByRole("alert")).toHaveTextContent("Failed to save user.");
});

it("shows unavailable details and audit history instead of an empty success", async () => {
  mocks.details.mockReturnValue({ error: new Error("Unavailable") });
  mocks.audit.mockReturnValue({ error: new Error("Unavailable") });
  await openMenu();
  await userEvent.click(screen.getByRole("menuitem", { name: "View details" }));
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Unable to load user details.",
  );
  await userEvent.click(screen.getByRole("button", { name: "Close" }));
  await userEvent.click(screen.getByRole("button", { name: /Tasks for/ }));
  await userEvent.click(
    screen.getByRole("menuitem", { name: "View audit events" }),
  );
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Unable to load audit events.",
  );
});
