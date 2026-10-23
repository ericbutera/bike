import { QueryClient } from "@tanstack/react-query";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { authApiClient, useAuth } from "../../lib/auth";
import { segmentTenants } from "../../tests/fixtures/segmentTenants";
import Providers from "../Providers";
import RequireAuth from "../RequireAuth";
import SegmentPersonalAchievements from "../segment-detail/SegmentPersonalAchievements";
import SegmentOverallAchievements from "../segment-detail/SegmentOverallAchievements";

type Scenario = ReturnType<typeof segmentTenants>;
type Viewer = Scenario["creator"] | null;
const currentUserKey = ["get", "/auth/current"] as const;
const personalKey = ["get", "/segments/{id}/personal-achievements"] as const;
const clients: QueryClient[] = [];

function SessionPanels() {
  const { user } = useAuth();
  const logout = authApiClient.useLogout();
  return (
    <>
      <p>{user ? `Signed in: ${user.name}` : "Signed out"}</p>
      {user && <button onClick={() => logout.mutate()}>Sign out</button>}
      <RequireAuth>
        <SegmentPersonalAchievements segmentId={1} />
        <SegmentOverallAchievements segmentId={1} />
      </RequireAuth>
    </>
  );
}

function session(viewerName: "creator" | "contributor" | "spectator") {
  const scenario = segmentTenants();
  let viewer: Viewer = scenario[viewerName];
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: 300_000 } },
  });
  clients.push(client);
  let holdPersonal: ((response: Response) => void) | undefined;
  let personalStarted = false;
  const pendingPersonal = new Promise<Response>((resolve) => {
    holdPersonal = resolve;
  });
  let delayPersonal = false;
  let heldResponse: Response;
  const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
    const url = new URL(input instanceof Request ? input.url : String(input));
    if (url.pathname.endsWith("/auth/current")) return Response.json(viewer);
    if (url.pathname.endsWith("/auth/logout")) {
      viewer = null;
      return Response.json({});
    }
    if (url.pathname.endsWith("/preferences")) return Response.json({});
    if (url.pathname.endsWith("/personal-achievements")) {
      const response = Response.json(scenario.personalFor(viewer, 1));
      if (delayPersonal) {
        delayPersonal = false;
        personalStarted = true;
        heldResponse = response;
        return pendingPersonal;
      }
      return response;
    }
    if (url.pathname.endsWith("/overall-achievements"))
      return Response.json(scenario.overallFor(1));
    throw new Error(`Unexpected tenant test request: ${url.pathname}`);
  });
  vi.stubGlobal("fetch", fetchMock);
  return {
    client,
    render: () =>
      render(
        <Providers
          client={client}
          config={{
            API_URL: "http://bike.test/api",
            MAP_STYLE_URL: "opentopomap",
            LOCAL_ADMIN_AUTO_LOGIN: false,
          }}
        >
          <SessionPanels />
        </Providers>,
      ),
    switchTo: async (next: "creator" | "contributor" | "spectator") => {
      viewer = scenario[next];
      await act(async () => {
        await client.invalidateQueries({ queryKey: currentUserKey });
      });
    },
    holdNextPersonal: () => {
      delayPersonal = true;
    },
    personalStarted: () => personalStarted,
    releasePersonal: async () => {
      await act(async () => {
        holdPersonal?.(heldResponse);
        await pendingPersonal;
      });
    },
  };
}

async function personalLinks(expectedIds: number[]) {
  await waitFor(() => {
    const panel = screen.getByRole("region", { name: "Personal achievements" });
    expect(
      within(panel)
        .getAllByRole("link")
        .map((link) => link.getAttribute("href")),
    ).toEqual(expectedIds.map((id) => `/segments/1?efforts=${id}`));
  });
}

async function bothOverallAthletes() {
  const table = await screen.findByRole("table", { name: "Overall top ten" });
  expect(
    within(table).getByRole("cell", { name: "Rider 7" }),
  ).toBeInTheDocument();
  expect(
    within(table).getByRole("cell", { name: "Rider 8" }),
  ).toBeInTheDocument();
}

afterEach(() => {
  cleanup();
  clients.splice(0).forEach((client) => client.clear());
  vi.unstubAllGlobals();
});

describe("personal results and global standings across three users", () => {
  it("shows the creator's own attempts alongside both athletes' global results", async () => {
    session("creator").render();
    await personalLinks([1, 2]);
    await bothOverallAthletes();
  });

  it("shows the contributor's own attempt alongside both athletes' global results", async () => {
    session("contributor").render();
    await personalLinks([3]);
    await bothOverallAthletes();
  });

  it("shows no personal attempts for the spectator while retaining both global athletes", async () => {
    session("spectator").render();
    expect(
      await screen.findByText("No personal achievements yet."),
    ).toBeInTheDocument();
    await bothOverallAthletes();
  });

  it("replaces the previous user's cached personal results when the account changes", async () => {
    const fixture = session("creator");
    fixture.render();
    await personalLinks([1, 2]);
    await fixture.switchTo("contributor");
    await personalLinks([3]);
  });

  it("does not reuse a contributing user's personal positions for a spectator", async () => {
    const fixture = session("contributor");
    fixture.render();
    await personalLinks([3]);
    await fixture.switchTo("spectator");
    expect(
      await screen.findByText("No personal achievements yet."),
    ).toBeInTheDocument();
    await bothOverallAthletes();
  });

  it("discards cached personal results after successful logout", async () => {
    const fixture = session("creator");
    fixture.render();
    await personalLinks([1, 2]);
    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));
    await screen.findByText("Signed out");
    await waitFor(() =>
      expect(
        fixture.client.getQueryCache().findAll({ queryKey: personalKey }),
      ).toHaveLength(0),
    );
  });

  it("does not let a late personal response from the previous account populate the next account", async () => {
    const fixture = session("creator");
    fixture.holdNextPersonal();
    fixture.render();
    await waitFor(() => expect(fixture.personalStarted()).toBe(true));
    await fixture.switchTo("contributor");
    await personalLinks([3]);
    await fixture.releasePersonal();
    await personalLinks([3]);
  });
});
