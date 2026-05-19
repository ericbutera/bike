import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import IntegrationEventFeed from "../IntegrationEventFeed";

describe("IntegrationEventFeed", () => {
  it("renders Strava events in a data grid", () => {
    render(
      <IntegrationEventFeed
        events={[
          {
            id: 1,
            user_id: 17,
            provider: "strava",
            event_type: "sync.completed",
            level: "success",
            message: "Imported 12 rides from Strava.",
            connection_id: 99,
            payload: {
              imported_count: 12,
              duplicate_count: 2,
              athlete_id: 555,
            },
            created_at: "2026-05-18T12:00:00Z",
          },
        ]}
        isLoading={false}
        error={null}
        emptyMessage="No events"
        showUserId
        showProvider
      />,
    );

    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.getByText("When")).toBeInTheDocument();
    expect(screen.getByText("Provider")).toBeInTheDocument();
    expect(screen.getByText("User")).toBeInTheDocument();
    expect(screen.getByText("Connection")).toBeInTheDocument();
    expect(
      screen.getByText("Imported 12 rides from Strava."),
    ).toBeInTheDocument();
    expect(screen.getByText("Imported: 12")).toBeInTheDocument();
    expect(screen.getByText("Duplicates: 2")).toBeInTheDocument();
    expect(screen.getByText("Athlete: 555")).toBeInTheDocument();
    expect(screen.getByText("User 17")).toBeInTheDocument();
    expect(screen.getByText("Connection 99")).toBeInTheDocument();
    expect(screen.getByText(/"athlete_id": 555/)).toBeInTheDocument();
  });

  it("shows the empty state when there are no events", () => {
    render(
      <IntegrationEventFeed
        events={[]}
        isLoading={false}
        error={null}
        emptyMessage="No events"
      />,
    );

    expect(screen.getByText("No events")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });
});
