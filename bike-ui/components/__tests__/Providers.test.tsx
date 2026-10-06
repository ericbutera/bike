import { render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { OAuthSignIn } from "../../lib/auth";
import Providers from "../Providers";

vi.mock("../../lib/unitPreferences", () => ({
  UnitPreferencesProvider: ({ children }: { children: React.ReactNode }) =>
    children,
}));

afterEach(() => vi.unstubAllGlobals());

it.each(["/api", "https://api.example.test/api/"])(
  "uses full-document OAuth navigation for API base %s",
  async (apiUrl) => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          providers: [{ id: "sso", label: "Continue with SSO" }],
        }),
      }),
    );
    render(
      <Providers
        config={{
          API_URL: apiUrl,
          MAP_STYLE_URL: "opentopomap",
          LOCAL_ADMIN_AUTO_LOGIN: false,
        }}
      >
        <OAuthSignIn />
      </Providers>,
    );
    expect(
      await screen.findByRole("link", { name: "Continue with SSO" }),
    ).toHaveAttribute("href", `${apiUrl.replace(/\/$/, "")}/oauth/sso`);
  },
);
