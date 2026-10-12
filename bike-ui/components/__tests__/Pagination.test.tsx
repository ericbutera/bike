import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import Pagination from "../ui/Pagination";

function PaginatedList() {
  const [page, setPage] = useState(1);
  return (
    <Pagination page={page} perPage={10} total={101} onPageChange={setPage} />
  );
}

describe("Pagination", () => {
  it("returns directly to page one after jumping to page five", async () => {
    const user = userEvent.setup();
    render(<PaginatedList />);

    await user.click(screen.getByRole("button", { name: "5" }));
    expect(screen.getByRole("button", { name: "5" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.queryByRole("button", { name: "1" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "First page" }));
    expect(screen.getByRole("button", { name: "1" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(
      screen.queryByRole("button", { name: "First page" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Previous page" }),
    ).not.toBeInTheDocument();
  });

  it.each([
    ["First page", "«", 1],
    ["Previous page", "‹", 4],
    ["Next page", "›", 6],
    ["Last page", "»", 11],
  ])("%s navigates to the correct page", async (label, arrow, target) => {
    const user = userEvent.setup();
    const onPageChange = vi.fn();
    render(
      <Pagination
        page={5}
        perPage={10}
        total={101}
        onPageChange={onPageChange}
      />,
    );

    const button = screen.getByRole("button", { name: label });
    expect(button).toHaveTextContent(arrow);
    await user.click(button);
    expect(onPageChange).toHaveBeenCalledExactlyOnceWith(target);
  });

  it.each([
    [1, ["1", "2", "3", "4", "5", "›", "»"]],
    [5, ["«", "‹", "3", "4", "5", "6", "7", "›", "»"]],
    [11, ["«", "‹", "7", "8", "9", "10", "11"]],
  ])("shows only available arrows on page %i", (page, buttons) => {
    render(
      <Pagination
        page={page}
        perPage={10}
        total={101}
        onPageChange={vi.fn()}
      />,
    );
    expect(
      screen.getAllByRole("button").map((button) => button.textContent),
    ).toEqual(buttons);
    expect(screen.getByRole("button", { name: String(page) })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it.each([1, 3, 5])(
    "hides every arrow when all pages are visible on page %i",
    (page) => {
      render(
        <Pagination
          page={page}
          perPage={10}
          total={50}
          onPageChange={vi.fn()}
        />,
      );
      expect(
        screen.getAllByRole("button").map((button) => button.textContent),
      ).toEqual(["1", "2", "3", "4", "5"]);
    },
  );

  it("uses the configured number of visible pages to hide arrows", () => {
    render(
      <Pagination
        page={5}
        perPage={10}
        total={101}
        visiblePages={11}
        onPageChange={vi.fn()}
      />,
    );
    expect(screen.getAllByRole("button")).toHaveLength(11);
    expect(
      screen.queryByRole("button", { name: /page/i }),
    ).not.toBeInTheDocument();
  });

  it("ignores the current page and allows selecting another visible page", async () => {
    const user = userEvent.setup();
    const onPageChange = vi.fn();
    render(
      <Pagination
        page={3}
        perPage={10}
        total={50}
        onPageChange={onPageChange}
      />,
    );
    await user.click(screen.getByRole("button", { name: "3" }));
    expect(onPageChange).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "1" }));
    expect(onPageChange).toHaveBeenCalledExactlyOnceWith(1);
  });

  it.each([0, 10])(
    "hides pagination when %i items fit on one page",
    (total) => {
      render(
        <Pagination
          page={1}
          perPage={10}
          total={total}
          onPageChange={vi.fn()}
        />,
      );
      expect(screen.queryByRole("button")).not.toBeInTheDocument();
    },
  );
});
