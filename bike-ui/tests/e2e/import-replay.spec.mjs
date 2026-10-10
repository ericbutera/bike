import { expect, test } from "@playwright/test";
import { fakeProductApi } from "./helpers/product-fixtures.mjs";
import { targets } from "./helpers/targets.mjs";
import { stabilize } from "./helpers/ui.mjs";

test.use({ channel: "chromium" });

function importFixture() {
  const labels = [
    "Raw stored",
    "Activity parsed",
    "Activity saved",
    "Segments built",
    "Segment analytics built",
    "Activity analytics built",
    "Training analysis built",
  ];
  const ids = [
    "raw_stored",
    "activity_parsed",
    "activity_saved",
    "segments_built",
    "segment_analytics_built",
    "activity_analytics_built",
    "training_analysis_built",
  ];
  const facts = [
    "GPX · 4096 bytes",
    "Parsed 7 records",
    "Saved activity #42",
    "0 affected segments",
    "Rebuilt analytics for 0 segments",
    "Rebuilt analytics for 1 activity",
    "Rebuilt training analysis for 1 activity",
  ];
  const nodes = ids.map((id, index) => ({
    id,
    stage: id,
    label: labels[index],
    status: "completed",
    summary: [facts[index]],
    completed_at: "2026-10-08T12:00:00Z",
  }));
  const item = {
    id: 17,
    import_version: 2,
    source: "manual_upload",
    status: "processed",
    original_filename: "synthetic-ride.gpx",
    format: "gpx",
    processing_stage: "complete",
    size_bytes: 4096,
    activity_id: 42,
    created_at: "2026-10-08T12:00:00Z",
  };
  const first = {
    id: 1,
    status: "completed",
    requested_stage: "raw_stored",
    start_stage: "raw_stored",
    source: {
      filename: item.original_filename,
      format: "gpx",
      quality: "gpx_original",
    },
    nodes,
    created_at: item.created_at,
  };
  const mermaid = [
    "flowchart LR",
    ...nodes.map((node) => `${node.id}["${node.label}"]`),
    ...ids.slice(1).map((id, index) => `${ids[index]} --> ${id}`),
  ].join("\n");
  return {
    item,
    trace: {
      import: item,
      nodes,
      graph: { nodes, edges: [], mermaid },
      attempts: [first],
      events: [],
    },
  };
}

async function fixture(page, target) {
  const diagnostics = [];
  page.on("console", (message) => {
    if (["warning", "error"].includes(message.type()))
      diagnostics.push(message.text());
  });
  page.on("pageerror", (error) => diagnostics.push(error.message));
  const api = await fakeProductApi(page, target);
  const state = importFixture();
  const replays = [];
  await page.route("**/api/activity-imports/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.pathname.endsWith("/history")) {
      await route.fulfill({
        json: { page: 1, per_page: 25, total: 1, items: [state.item] },
      });
    } else if (url.pathname.endsWith("/trace")) {
      await route.fulfill({ json: state.trace });
    } else if (url.pathname.endsWith("/replay")) {
      if (request.method() === "GET") {
        const stage = url.searchParams.get("stage");
        await route.fulfill({
          json: {
            requested_stage: stage,
            start_stage: stage,
            reused_attempt_id: 1,
          },
        });
      } else {
        const body = request.postDataJSON();
        replays.push(body);
        const nodes = state.trace.nodes.map((node, index) => ({
          ...node,
          status: index < 2 ? "reused" : "completed",
          reused_attempt_id: index < 2 ? 1 : null,
        }));
        state.trace.attempts.unshift({
          ...state.trace.attempts[0],
          id: 2,
          requested_stage: body.stage,
          start_stage: body.stage,
          reused_attempt_id: 1,
          nodes,
        });
        await route.fulfill({
          status: 202,
          json: {
            requested_stage: body.stage,
            start_stage: body.stage,
            reused_attempt_id: 1,
            attempt_id: 2,
          },
        });
      }
    } else {
      throw new Error(`Unexpected import request: ${request.url()}`);
    }
  });
  return { state, api, diagnostics, replays };
}

for (const target of targets) {
  test(`${target.name}: import history exposes summaries and replays from a DAG node`, async ({
    page,
  }) => {
    const { api, diagnostics, replays } = await fixture(page, target);
    await page.goto(new URL("/imports", target.url).toString());
    await expect(
      page.getByRole("heading", { name: "Imports", exact: true }),
    ).toBeVisible();
    await page
      .getByRole("link", { name: "synthetic-ride.gpx", exact: true })
      .click();
    await expect(
      page.getByRole("heading", { name: "synthetic-ride.gpx", exact: true }),
    ).toBeVisible();
    const graph = page.locator("svg g.node");
    await expect(graph).toHaveCount(7);
    await expect(
      page.getByRole("button", { name: /Activity parsed completed/ }),
    ).toContainText("Parsed 7 records");
    await expect(graph.first()).toContainText("4096 bytes");
    const savedNode = graph.filter({ hasText: "Activity saved" });
    expect(await savedNode.getAttribute("id")).toMatch(
      /flowchart-activity_saved-\d+$/,
    );
    await savedNode.click();
    await expect(
      page.getByRole("button", { name: /Activity saved completed/ }),
    ).toHaveAttribute("aria-pressed", "true");
    await page
      .getByRole("button", { name: "Replay from Activity saved", exact: true })
      .click();
    await expect(page.getByRole("combobox", { name: "Attempt" })).toHaveValue(
      "2",
    );
    await expect(
      page.getByRole("button", { name: /Raw stored reused/ }),
    ).toContainText("Reused from attempt #1");
    expect(replays).toEqual([
      {
        stage: "activity_saved",
        expected_start_stage: "activity_saved",
        expected_reused_attempt_id: 1,
      },
    ]);
    await page.getByRole("combobox", { name: "Attempt" }).selectOption("1");
    await expect(
      page.getByRole("button", { name: /Raw stored completed/ }),
    ).toBeVisible();
    expect(api.unexpected).toEqual([]);
    expect(api.requests.filter((path) => path.startsWith("/admin/"))).toEqual(
      [],
    );
    expect(diagnostics).toEqual([]);
    await stabilize(page);
    await page.screenshot({
      path: ".artifacts/playwright/import-replay.png",
      fullPage: true,
    });
  });

  test(`${target.name}: a failed import without an activity offers replay`, async ({
    page,
  }) => {
    const { state, api, diagnostics } = await fixture(page, target);
    state.item.status = "failed";
    state.item.activity_id = null;
    state.item.processing_stage = "activity_parsed";
    state.trace.attempts[0].status = "failed";
    state.trace.attempts[0].error = "Invalid GPX recording";
    state.trace.nodes[1].status = "failed";
    state.trace.nodes[1].error = "Invalid GPX recording";
    await page.goto(new URL("/imports", target.url).toString());
    await expect(page.getByText("No activity yet")).toBeVisible();
    await page
      .getByRole("link", { name: "synthetic-ride.gpx", exact: true })
      .click();
    await expect(
      page.getByRole("button", {
        name: "Replay from Activity parsed",
        exact: true,
      }),
    ).toBeEnabled();
    await expect(page.getByRole("link", { name: "View activity" })).toHaveCount(
      0,
    );
    expect(api.unexpected).toEqual([]);
    expect(diagnostics).toEqual([]);
  });
}
