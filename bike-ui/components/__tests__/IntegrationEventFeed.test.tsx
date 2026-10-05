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

  it("labels a gateway's Strava activity id separately", () => {
    render(
      <IntegrationEventFeed
        events={[
          {
            id: 2,
            user_id: 17,
            provider: "strava",
            event_type: "gateway.delivery.applied",
            level: "success",
            message: "Applied Strava gateway upsert delivery.",
            payload: { strava_activity_id: 9876543210 },
            created_at: "2026-10-05T12:00:00Z",
          },
        ]}
        isLoading={false}
        error={null}
        emptyMessage="No events"
      />,
    );
    expect(screen.getByText("Strava activity: 9876543210")).toBeInTheDocument();
  });

  it("shows a visible error when integration history cannot load", () => {
    render(
      <IntegrationEventFeed
        events={[]}
        isLoading={false}
        error={new Error("API unavailable")}
        emptyMessage="No events"
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Unable to load integration events.",
    );
    expect(screen.queryByText("No events")).not.toBeInTheDocument();
  });
});
