import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { initializeClientConfig } from "../../lib/config";

afterEach(() => initializeClientConfig({}));
beforeEach(() => {
  vi.spyOn(HTMLDialogElement.prototype, "showModal").mockImplementation(
    function (this: HTMLDialogElement) {
      this.setAttribute("open", "");
    },
  );
});
afterEach(() => vi.restoreAllMocks());
import type { IntegrationEvent } from "@/lib/queries";
import IntegrationEventFeed from "../IntegrationEventFeed";

const accepted: IntegrationEvent = {
  id: 1,
  user_id: 17,
  provider: "strava",
  event_type: "strava.event.accepted",
  level: "info",
  message: "Strava Sync requested.",
  connection_id: 2,
  created_at: "2026-10-13T12:00:00Z",
  payload: {
    bike_customer_id: "customer",
    strava_athlete_id: 555,
    strava_activity_id: 99133701,
    temporal_namespace: "bike",
    temporal_workflow_id: "strava-event/with space",
  },
};

describe("IntegrationEventFeed", () => {
  it("inspects event details and the native Temporal execution from a compact icon", async () => {
    initializeClientConfig({
      TEMPORAL_UI_URL: "https://temporal.example.com/",
    });
    render(
      <IntegrationEventFeed
        audience="admin"
        events={[accepted]}
        isLoading={false}
        error={null}
        emptyMessage="No events"
        showUserId
        showProvider
      />,
    );
    expect(screen.queryByText(accepted.message)).not.toBeInTheDocument();
    expect(
      screen
        .getAllByRole("columnheader")
        .slice(-2)
        .map((cell) => cell.textContent),
    ).toEqual(["When", "Inspect"]);
    const eventCells = screen.getAllByRole("cell");
    expect(eventCells.at(-2)).toHaveTextContent("2026");
    expect(
      within(eventCells.at(-1)!).getByRole("button", {
        name: "Inspect event 1",
      }),
    ).toBeVisible();
    expect(screen.getByText("User 17")).toBeInTheDocument();
    for (const label of ["User 17", "Connection 2", "Strava Event Accepted"]) {
      expect(screen.getByText(label).className).not.toContain("badge");
    }
    expect(screen.getByRole("img", { name: "Strava" })).toHaveClass(
      "h-4",
      "w-4",
    );
    expect(
      screen.queryByRole("columnheader", { name: "Details" }),
    ).not.toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Inspect event 1" }),
    );
    expect(
      within(screen.getByRole("dialog")).getByText(accepted.message),
    ).toBeVisible();
    expect(
      screen.getByRole("link", { name: "View job in Temporal" }),
    ).toHaveAttribute(
      "href",
      "https://temporal.example.com/namespaces/bike/workflows/strava-event%2Fwith%20space",
    );
    await userEvent.click(screen.getByText("Payload", { exact: true }));
    expect(screen.getByText(/"strava_athlete_id": 555/)).toBeVisible();
    await userEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("shows account requests without claiming workflow completion", () => {
    render(
      <IntegrationEventFeed
        audience="account"
        events={[accepted]}
        isLoading={false}
        error={null}
        emptyMessage="No events"
      />,
    );
    expect(screen.getByText(accepted.message)).toBeInTheDocument();
    expect(screen.queryByText(/completed/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(screen.queryByText("customer")).not.toBeInTheDocument();
  });

  it("shows empty, loading and error states", () => {
    const props = {
      events: [],
      isLoading: false,
      error: null,
      emptyMessage: "No events",
    };
    const { rerender } = render(<IntegrationEventFeed {...props} />);
    expect(screen.getByText("No events")).toBeInTheDocument();
    rerender(<IntegrationEventFeed {...props} isLoading />);
    expect(screen.queryByText("No events")).not.toBeInTheDocument();
    rerender(
      <IntegrationEventFeed {...props} error={new Error("Fixture failure")} />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Unable to load Strava event history.",
    );
  });
});
