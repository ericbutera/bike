import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import TimeRangeSelector from "./TimeRangeSelector";

describe("report interval selection", () => {
  it("displays the selected interval and sends the rider's next selection", () => {
    const onChange = vi.fn();
    render(<TimeRangeSelector value="month" onChange={onChange} />);
    const select = screen.getByRole("combobox", { name: "Interval" });
    expect(select).toHaveValue("month");

    fireEvent.change(select, { target: { value: "6month" } });
    expect(onChange).toHaveBeenCalledExactlyOnceWith("6month");
  });
});
