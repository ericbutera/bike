import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
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
  it("links an accepted event to its native Temporal execution", () => {
    render(
      <IntegrationEventFeed
        audience="admin"
        events={[accepted]}
        isLoading={false}
        error={null}
        emptyMessage="No events"
        showUserId
      />,
    );
    expect(screen.getByText(accepted.message)).toBeInTheDocument();
    expect(screen.getByText("User 17")).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "View job in Temporal" }),
    ).toHaveAttribute(
      "href",
      "http://localhost:8080/namespaces/bike/workflows/strava-event%2Fwith%20space",
    );
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
