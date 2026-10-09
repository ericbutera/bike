import { connectedTest as test, expect } from "./helpers/test.mjs";
import { openActivityList, selectTheme } from "./helpers/ui.mjs";
import { targets } from "./helpers/targets.mjs";

test("activity list renders and light theme persistence works in Bike", async ({
  createContext,
}) => {
  for (const target of targets) {
    const context = await createContext({ colorScheme: "light" });
    const page = await context.newPage();

    await openActivityList(page, target);
    await selectTheme(page, "light");
    await page.reload({ waitUntil: "domcontentloaded" });
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

    await context.close();
  }
});
