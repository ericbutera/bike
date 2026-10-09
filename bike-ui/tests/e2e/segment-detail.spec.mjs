import { connectedTest as test, expect } from "./helpers/test.mjs";
import { expectedEffortCount, segmentId, targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

test("segment detail renders the shared workspace and supports core interactions", async ({
  createContext,
}) => {
  const rustFilterSummaries = new Map();
  for (const target of targets) {
    const context = await createContext({ colorScheme: "light" });
    const page = await context.newPage();
    const segmentRequests = [];

    page.on("request", (request) => {
      if (request.url().includes("/api/segments/")) {
        segmentRequests.push(request.url());
      }
    });

    await openRoute(page, target, `/segments/${segmentId}`);
    await expect(page.getByRole("heading", { name: "Efforts" })).toBeVisible();
    await expect(page.getByLabel("Segment efforts table")).toBeVisible();
    const effortsCard = page
      .getByRole("heading", { name: "Efforts" })
      .locator(
        "xpath=ancestor::*[contains(concat(' ', normalize-space(@class), ' '), ' card ')][1]",
      );
    await expect(
      page.getByText(
        `Showing 1-${Math.min(10, expectedEffortCount)} of ${expectedEffortCount} efforts`,
      ),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Comparison workspace" }).first(),
    ).toBeVisible();
    await expect(
      page.locator('[aria-label="Segment comparison map"]'),
    ).toBeVisible();
    await expect(
      page.locator('[aria-label="Segment comparison chart"]'),
    ).toBeVisible();
    await expect(
      page.locator('[aria-label="Playback timeline"]'),
    ).toBeVisible();
    await expect(
      page.getByRole("link", { name: "Open race viewer" }),
    ).toHaveAttribute(
      "href",
      new RegExp(`/segments/${segmentId}/race\\?efforts=`),
    );

    expect(
      segmentRequests.some((url) => /\/segments\/0(?:\/|$)/.test(url)),
      `${target.name} requested an invalid segment id: ${segmentRequests.join(", ")}`,
    ).toBe(false);

    if (expectedEffortCount > 10) {
      const pageCount = Math.ceil(expectedEffortCount / 10);
      for (let pageNumber = 2; pageNumber <= pageCount; pageNumber += 1) {
        await effortsCard.getByRole("button", { name: "Next page" }).click();
        await expect(
          page.getByText(
            `Showing ${(pageNumber - 1) * 10 + 1}-${Math.min(pageNumber * 10, expectedEffortCount)} of ${expectedEffortCount} efforts`,
          ),
        ).toBeVisible();
        await expect(
          effortsCard.getByRole("button", {
            name: String(pageNumber),
            exact: true,
          }),
        ).toHaveAttribute("aria-current", "page");
      }
      for (let pageNumber = pageCount - 1; pageNumber >= 1; pageNumber -= 1) {
        await effortsCard
          .getByRole("button", { name: "Previous page" })
          .click();
        await expect(
          page.getByText(
            `Showing ${(pageNumber - 1) * 10 + 1}-${Math.min(pageNumber * 10, expectedEffortCount)} of ${expectedEffortCount} efforts`,
          ),
        ).toBeVisible();
      }
    } else {
      await page
        .getByRole("button", { name: /^Remove .* from comparison$/ })
        .first()
        .click();
    }

    const addButton = page
      .getByRole("button", { name: /^Add .* to comparison$/ })
      .first();
    const addLabel = await addButton.getAttribute("aria-label");
    const rowIndex = await addButton
      .locator("xpath=ancestor::tr")
      .evaluate((row) => row.sectionRowIndex);
    const effortRow = page
      .getByLabel("Segment efforts table")
      .locator("tbody tr")
      .nth(rowIndex);
    await addButton.focus();
    await expect(addButton).toBeFocused();
    await page.keyboard.press("Enter");
    const removeButton = effortRow.getByRole("button", {
      name: addLabel.replace(
        /^Add (.*) to comparison$/,
        "Remove $1 from comparison",
      ),
    });
    await expect(removeButton).toBeVisible();
    await removeButton.focus();
    await expect(removeButton).toBeFocused();
    await page.keyboard.press("Enter");
    const restoredAdd = effortRow.getByRole("button", {
      name: addLabel,
      exact: true,
    });
    await expect(restoredAdd).toBeVisible();
    await restoredAdd.click();

    const chart = page.getByRole("img", { name: "Segment comparison chart" });
    const surface = chart.locator(".recharts-surface");
    await expect(surface).toBeVisible();
    const bounds = await surface.boundingBox();
    await surface.hover({
      position: { x: bounds.width / 2, y: bounds.height / 3 },
    });
    const tooltip = chart.locator(".recharts-tooltip-wrapper");
    await expect(tooltip).toBeVisible();
    await expect(
      tooltip.getByLabel(/^Ride .* tooltip row$/).first(),
    ).toBeVisible();

    const timeline = page.locator('[aria-label="Playback timeline"]');
    await timeline.fill("50");
    await expect(timeline).toHaveValue("50");

    const summary = effortsCard.getByText(
      /^(?:Showing \d+-\d+ of \d+ efforts|No efforts in this time window)$/,
    );
    for (const filterName of ["Day", "Week", "Month", "Year", "All"]) {
      const filter = effortsCard.getByRole("button", {
        name: filterName,
        exact: true,
      });
      await filter.click();
      await expect(filter).toHaveClass(/btn-neutral/);
      await expect(summary).toBeVisible();
      const actual = await summary.textContent();
      if (target.name === "rust") {
        rustFilterSummaries.set(filterName, actual);
      } else {
        expect(actual, `${target.name} ${filterName} effort filter`).toBe(
          rustFilterSummaries.get(filterName),
        );
      }
    }
    await expect(summary).toHaveText(
      `Showing 1-${Math.min(10, expectedEffortCount)} of ${expectedEffortCount} efforts`,
    );

    await context.close();
  }
});
