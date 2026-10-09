import { test, expect } from "./helpers/test.mjs";
import { fakeProductApi } from "./helpers/product-fixtures.mjs";
import { targets } from "./helpers/targets.mjs";

for (const target of targets) {
  test(`${target.name}: sign-in return, session reload, and logout work with a fake auth boundary`, async ({
    page,
  }) => {
    const state = await fakeProductApi(page, target, { signedIn: false });
    await page.goto(new URL("/login", target.url).toString());
    await expect(
      page.getByRole("heading", { name: "Sign in with OAuth/OIDC" }),
    ).toBeVisible();
    await page
      .getByRole("link", { name: "Continue with SSO", exact: true })
      .click();
    await expect(
      page.getByRole("heading", { name: "Recent activities" }),
    ).toBeVisible();
    await page.reload();
    await expect(
      page.getByRole("heading", { name: "Recent activities" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Account", exact: true }).click();
    await page.getByRole("button", { name: "Sign out", exact: true }).click();
    await expect(
      page.getByRole("link", { name: "Sign in", exact: true }).first(),
    ).toBeVisible();
    expect(state.requests).toContain("/oauth/sso");
    expect(state.requests).toContain("/auth/logout");
    expect(state.signedIn).toBe(false);
    expect(state.unexpected).toEqual([]);
  });
}
