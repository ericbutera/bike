import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ImportsPage from "./page";
import ImportPage from "./[id]/page";

vi.mock("../../components/imports/ImportHistory", () => ({
  default: ({ archiveJobId }: { archiveJobId?: number }) => (
    <p>{archiveJobId ? `Archive ${archiveJobId}` : "All imports"}</p>
  ),
}));
vi.mock("../../components/imports/ImportDetail", () => ({
  default: ({ importId }: { importId: string }) => <p>Import {importId}</p>,
}));

describe("import page inputs", () => {
  it("opens the history for a selected archive job", async () => {
    const page = await ImportsPage({
      searchParams: Promise.resolve({ archive_job_id: "9" }),
    });
    render(page);
    expect(screen.getByText("Archive 9")).toBeInTheDocument();
  });

  it("opens all history when an archive filter is invalid", async () => {
    const page = await ImportsPage({
      searchParams: Promise.resolve({ archive_job_id: "invalid" }),
    });
    render(page);
    expect(screen.getByText("All imports")).toBeInTheDocument();
  });

  it("passes the selected import to its detail view", async () => {
    const page = await ImportPage({ params: Promise.resolve({ id: "17" }) });
    render(page);
    expect(screen.getByText("Import 17")).toBeInTheDocument();
  });
});
