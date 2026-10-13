import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { formatActivityTimestamp } from "@/lib/activityFormatting";
import type { IntegrationEvent } from "@/lib/queries";
import IntegrationEventFeed from "../IntegrationEventFeed";

const mocks = vi.hoisted(() => ({ activity: vi.fn() }));
vi.mock("@/lib/queries", () => ({ useActivity: mocks.activity }));

function stravaEvent(
  payload: IntegrationEvent["payload"],
  overrides: Partial<IntegrationEvent> = {},
): IntegrationEvent {
  return {
    id: 1,
    user_id: 17,
    provider: "strava",
    event_type: "workflow.stage",
    level: "info",
    message: "Strava sync: complete completed.",
    connection_id: 2,
    payload: {
      athlete_id: 555,
      workflow_id: "internal-workflow",
      import_id: 88,
      ...payload,
    },
    created_at: "2026-10-12T12:00:00Z",
    ...overrides,
  };
}

beforeEach(() => {
  mocks.activity.mockReset().mockReturnValue({ data: null, error: null });
});

describe("IntegrationEventFeed", () => {
  it("renders Strava events in a data grid", () => {
    render(
      <IntegrationEventFeed
        audience="admin"
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

  it("shows native Strava activity, workflow and import correlation", () => {
    render(
      <IntegrationEventFeed
        audience="admin"
        events={[
          {
            id: 2,
            user_id: 17,
            provider: "strava",
            event_type: "workflow.stage",
            level: "success",
            message: "Strava insert: apply applied.",
            payload: {
              strava_activity_id: 9876543210,
              workflow_id: "native-workflow",
              import_id: 17,
            },
            created_at: "2026-10-05T12:00:00Z",
          },
        ]}
        isLoading={false}
        error={null}
        emptyMessage="No events"
      />,
    );
    expect(screen.getByText("Strava activity: 9876543210")).toBeInTheDocument();
    expect(screen.getByText("Workflow: native-workflow")).toBeInTheDocument();
    expect(screen.getByText("Import: 17")).toBeInTheDocument();
  });

  it("shows a visible error when integration history cannot load", () => {
    render(
      <IntegrationEventFeed
        audience="admin"
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

  it("shows only dates and confirmed activity changes on accounts by default", () => {
    mocks.activity.mockImplementation((id) => ({
      data: id === 42 ? { title: "Morning ride" } : null,
      error: null,
    }));
    const changed = stravaEvent({
      stage: "apply",
      outcome: "applied",
      activity_id: 42,
      strava_activity_id: 9876543210,
      action: "upsert",
    });
    render(
      <IntegrationEventFeed
        events={[
          stravaEvent(
            { stage: "complete", outcome: "completed", action: "sync" },
            { id: 2 },
          ),
          stravaEvent({ ...changed.payload, stage: "derived" }, { id: 3 }),
          changed,
        ]}
        isLoading={false}
        error={null}
        emptyMessage="No recent activity changes to show."
      />,
    );

    expect(
      screen.getAllByRole("columnheader").map((cell) => cell.textContent),
    ).toEqual(["Date", "Activities modified"]);
    expect(screen.getAllByRole("row")).toHaveLength(3);
    expect(screen.getByText("Sync completed.")).toBeInTheDocument();
    for (const date of screen.getAllByText(
      formatActivityTimestamp(changed.created_at),
    )) {
      expect(date).toHaveAttribute("datetime", changed.created_at);
    }
    const activityLink = screen.getByRole("link", { name: "Morning ride" });
    expect(activityLink).toHaveAttribute("href", "/activities/42");
    expect(activityLink.closest("td")).toHaveTextContent(
      "Updated: Morning ride",
    );
    expect(
      screen.queryByText(/Strava sync:|Athlete:|Workflow:|Connection 2/),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(/Payload|Workflow Stage|internal-workflow|import_id/),
    ).not.toBeInTheDocument();
  });

  it("identifies removed activities without fetching or linking a deleted page", () => {
    render(
      <IntegrationEventFeed
        events={[
          stravaEvent({
            stage: "delete",
            outcome: "deleted",
            activity_id: 42,
            strava_activity_id: 9876543210,
          }),
        ]}
        isLoading={false}
        error={null}
        emptyMessage="No changes"
      />,
    );
    expect(
      screen.getByText("Strava activity 9876543210").closest("td"),
    ).toHaveTextContent("Removed: Strava activity 9876543210");
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(mocks.activity).toHaveBeenCalledWith(null);
  });

  it.each([
    ["apply", "unchanged"],
    ["apply", "excluded"],
    ["stale", "stale"],
    ["delete", "already_absent"],
    ["activity", "completed"],
    ["normalize", "completed"],
    ["derived", "applied"],
    ["complete", "completed"],
  ])("does not present %s / %s as an activity change", (stage, outcome) => {
    render(
      <IntegrationEventFeed
        events={[stravaEvent({ stage, outcome, activity_id: 42 })]}
        isLoading={false}
        error={null}
        emptyMessage="No recent activity changes to show."
      />,
    );
    expect(
      screen.getByText("No recent activity changes to show."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(mocks.activity).not.toHaveBeenCalled();
  });

  it("ignores failures, acceptance, other providers and missing activity identities", () => {
    const payload = { stage: "apply", outcome: "applied", activity_id: 42 };
    render(
      <IntegrationEventFeed
        events={[
          stravaEvent(payload, { event_type: "workflow.stage.failed" }),
          stravaEvent(payload, { id: 2, event_type: "workflow.accepted" }),
          stravaEvent(payload, { id: 3, provider: "other" }),
          stravaEvent({ stage: "apply", outcome: "applied" }, { id: 4 }),
          stravaEvent(null, { id: 5 }),
        ]}
        isLoading={false}
        error={null}
        emptyMessage="No changes"
      />,
    );
    expect(screen.getByText("No changes")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  it("uses an activity identity while its name is unavailable", () => {
    render(
      <IntegrationEventFeed
        events={[
          stravaEvent({ stage: "apply", outcome: "applied", activity_id: 42 }),
          stravaEvent(
            {
              stage: "apply",
              outcome: "applied",
              activity_id: -1,
              strava_activity_id: 9876543210,
            },
            { id: 2 },
          ),
        ]}
        isLoading={false}
        error={null}
        emptyMessage="No changes"
      />,
    );
    expect(screen.getByRole("link", { name: "Activity 42" })).toHaveAttribute(
      "href",
      "/activities/42",
    );
    expect(screen.getByText("Strava activity 9876543210")).toBeInTheDocument();
    expect(screen.getAllByRole("link")).toHaveLength(1);
  });

  it("keeps a retained name after removal", () => {
    render(
      <IntegrationEventFeed
        events={[
          stravaEvent({
            stage: "delete",
            outcome: "deleted",
            activity_id: 42,
            activity_name: "Evening ride",
          }),
        ]}
        isLoading={false}
        error={null}
        emptyMessage="No changes"
      />,
    );
    expect(screen.getByText("Evening ride").closest("td")).toHaveTextContent(
      "Removed: Evening ride",
    );
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  });

  it("avoids linking activities that are no longer available", () => {
    mocks.activity.mockReturnValue({
      data: null,
      error: new Error("Not found"),
    });
    render(
      <IntegrationEventFeed
        events={[
          stravaEvent({ stage: "apply", outcome: "applied", activity_id: 42 }),
        ]}
        isLoading={false}
        error={null}
        emptyMessage="No changes"
      />,
    );
    expect(screen.getByText("Activity 42")).toBeInTheDocument();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(screen.queryByText("Not found")).not.toBeInTheDocument();
  });

  it("shows loading and a plain error without leaking API diagnostics", () => {
    const { rerender } = render(
      <IntegrationEventFeed
        events={[]}
        isLoading
        error={null}
        emptyMessage="No changes"
      />,
    );
    expect(screen.getByRole("status", { name: "Loading" })).toBeInTheDocument();
    expect(screen.queryByText("No changes")).not.toBeInTheDocument();
    rerender(
      <IntegrationEventFeed
        events={[]}
        isLoading={false}
        error={new Error("Workflow internal-workflow failed")}
        emptyMessage="No changes"
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Unable to load Strava sync history.",
    );
    expect(screen.queryByText(/internal-workflow/)).not.toBeInTheDocument();
  });

  it("keeps a new queued sync and previous syncs visible without activity modifications", () => {
    render(
      <IntegrationEventFeed
        events={[
          stravaEvent(
            { action: "sync", workflow_id: "new-run" },
            { event_type: "workflow.accepted" },
          ),
          stravaEvent(
            {
              action: "sync",
              stage: "complete",
              outcome: "completed",
              workflow_id: "old-run",
            },
            { id: 2 },
          ),
          stravaEvent(
            { action: "sync", workflow_id: "old-run" },
            { id: 3, event_type: "workflow.accepted" },
          ),
          stravaEvent(null, {
            id: 4,
            event_type: "sync.succeeded",
            payload: { imported_count: 3 },
          }),
          stravaEvent(null, {
            id: 5,
            event_type: "sync_completed",
            payload: { imported_count: 0 },
          }),
        ]}
        isLoading={false}
        error={null}
        emptyMessage="No history"
      />,
    );
    expect(screen.getByText("Sync queued.")).toBeInTheDocument();
    expect(screen.getByText("Sync completed.")).toBeInTheDocument();
    expect(
      screen.getByText("Sync completed. 3 activities imported."),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Sync completed. 0 activities imported."),
    ).toBeInTheDocument();
    expect(screen.getAllByRole("row")).toHaveLength(5);
    expect(
      screen.queryByText(/internal-workflow|Athlete:|Workflow:|Payload/),
    ).not.toBeInTheDocument();
    expect(mocks.activity).not.toHaveBeenCalled();
  });

  it.each([
    ["sync.started", {}, "Sync in progress."],
    ["sync.failed", {}, "Sync needs attention."],
    ["sync.cancelled", {}, "Sync cancelled."],
    ["sync.paused", {}, "Sync paused."],
    ["sync.resumed", {}, "Sync in progress."],
    ["sync.future", {}, "Sync update."],
    [
      "sync.completed",
      { imported_count: 1 },
      "Sync completed. 1 activity imported.",
    ],
    ["sync.completed", { imported_count: -1 }, "Sync completed."],
    [
      "workflow.stage",
      { action: "backfill", stage: "discover" },
      "Sync in progress.",
    ],
    [
      "workflow.stage",
      { action: "sync", stage: "children" },
      "Sync in progress.",
    ],
    ["workflow.stage.failed", { stage: "children" }, "Sync needs attention."],
  ])("shows a plain sync state for %s", (event_type, payload, summary) => {
    render(
      <IntegrationEventFeed
        events={[stravaEvent(payload, { event_type })]}
        isLoading={false}
        error={null}
        emptyMessage="No history"
      />,
    );
    expect(screen.getByText(summary)).toBeInTheDocument();
    expect(
      screen.queryByText("Strava sync: complete completed."),
    ).not.toBeInTheDocument();
  });

  it("preserves older deletion history without confusing provider IDs with Bike IDs", () => {
    render(
      <IntegrationEventFeed
        events={[
          stravaEvent(
            { activity_id: 42 },
            { event_type: "webhook.activity_delete", level: "success" },
          ),
        ]}
        isLoading={false}
        error={null}
        emptyMessage="No history"
      />,
    );
    expect(
      screen.getByText("Strava activity 42").closest("td"),
    ).toHaveTextContent("Removed: Strava activity 42");
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(mocks.activity).toHaveBeenCalledWith(null);
  });
});
