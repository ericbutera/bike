import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, afterEach, expect, it, vi } from "vitest";
import { adminUser } from "@/tests/fixtures/adminTasks";
import { UserTaskMenu } from "../admin/AdminTaskMenu";

const mocks = vi.hoisted(() => ({
  importArchive: vi.fn(),
  cleanup: vi.fn(),
  archiveUser: vi.fn(),
}));
vi.mock("../../lib/queries", () => ({
  useAdminUserArchiveImport: (id: number) => {
    mocks.archiveUser(id);
    return { importAsync: mocks.importArchive, isPending: false };
  },
  useCleanupUserDuplicateActivities: () => ({
    cleanupAsync: mocks.cleanup,
    isPending: false,
  }),
}));
vi.mock("../../lib/adminTasks", () => ({ useAdminTask: () => ({}) }));
vi.mock("next/navigation", () => ({ useRouter: () => ({}) }));

beforeEach(() => {
  vi.clearAllMocks();
  mocks.importArchive.mockResolvedValue({ id: 20 });
  mocks.cleanup.mockResolvedValue({
    message: "Removed duplicates.",
    deleted_activity_count: 2,
    retained_activity_count: 3,
  });
  vi.spyOn(HTMLDialogElement.prototype, "showModal").mockImplementation(
    function (this: HTMLDialogElement) {
      this.setAttribute("open", "");
    },
  );
});
afterEach(() => vi.restoreAllMocks());

async function choose(label: string) {
  const rowClick = vi.fn();
  render(
    <div onClick={rowClick}>
      <UserTaskMenu user={adminUser()} />
    </div>,
  );
  await userEvent.click(screen.getByRole("button", { name: /Tasks for/ }));
  await userEvent.click(screen.getByText("Task management"));
  await userEvent.click(screen.getByRole("menuitem", { name: label }));
  expect(screen.getByRole("dialog")).toHaveTextContent("Rider (#7)");
  expect(rowClick).not.toHaveBeenCalled();
}

it("queues provider archive imports for the selected user", async () => {
  await choose("Fetch provider export archives");
  await userEvent.type(
    screen.getByLabelText("Archive URL"),
    "https://example.com/export.zip",
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Queue archive import" }),
  );
  expect(mocks.archiveUser).toHaveBeenLastCalledWith(7);
  expect(mocks.importArchive).toHaveBeenCalledWith(
    "https://example.com/export.zip",
  );
  expect(screen.getByRole("status")).toHaveTextContent(
    "Archive import queued for this user",
  );
  expect(
    screen.getByRole("link", { name: "View archive imports" }),
  ).toHaveAttribute("href", "/admin/imports?archive_job_id=20");
});

it("cleans duplicates for the row user without retyping an ID", async () => {
  await choose("Clean up duplicate activities");
  expect(screen.queryByRole("spinbutton")).not.toBeInTheDocument();
  await userEvent.click(
    screen.getByRole("button", { name: "Clean duplicates" }),
  );
  expect(mocks.cleanup).toHaveBeenCalledWith(7);
  expect(screen.getByRole("status")).toHaveTextContent(
    "Deleted: 2. Retained: 3.",
  );
});

it("keeps failed cleanup retryable and does not claim success", async () => {
  mocks.cleanup.mockRejectedValueOnce(new Error("Unavailable"));
  await choose("Clean up duplicate activities");
  await userEvent.click(
    screen.getByRole("button", { name: "Clean duplicates" }),
  );
  expect(screen.getByRole("alert")).toHaveTextContent("Unavailable");
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
  await userEvent.click(
    screen.getByRole("button", { name: "Clean duplicates" }),
  );
  expect(screen.getByRole("status")).toBeInTheDocument();
});
