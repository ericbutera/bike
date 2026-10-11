import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import ProcessingGraph from "../ui/ProcessingGraph";

describe("ProcessingGraph", () => {
  it("draws the recorded branches and keeps parallel stages on the same row", async () => {
    const onSelect = vi.fn();
    const { container } = render(
      <ProcessingGraph
        nodes={[
          { id: "saved", label: "Saved", status: "completed" },
          { id: "segments", label: "Segments", status: "failed" },
          { id: "training", label: "Training", status: "pending" },
        ]}
        edges={[
          { from: "saved", to: "segments" },
          { from: "saved", to: "training" },
        ]}
        selected="segments"
        onSelect={onSelect}
      />,
    );
    const segments = screen.getByRole("button", { name: "Segments failed" });
    const training = screen.getByRole("button", { name: "Training pending" });
    expect(segments.style.top).toBe(training.style.top);
    expect(segments.style.top).not.toBe(
      screen.getByRole("button", { name: "Saved completed" }).style.top,
    );
    expect(container.querySelectorAll("path[data-from]")).toHaveLength(2);
    expect(
      container.querySelector('path[data-from="segments"][data-to="training"]'),
    ).toBeNull();
    expect(segments).toHaveAttribute("aria-pressed", "true");
    training.focus();
    await userEvent.setup().keyboard("{Enter}");
    expect(onSelect).toHaveBeenCalledWith("training");
  });
});
