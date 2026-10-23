import { QueryClient } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { OAuthSignIn } from "../../lib/auth";
import Providers from "../Providers";
import RequireAuth from "../RequireAuth";

vi.mock("../../lib/unitPreferences", () => ({
  UnitPreferencesProvider: ({ children }: { children: React.ReactNode }) =>
    children,
}));

const clients: QueryClient[] = [];

afterEach(() => {
  cleanup();
  clients.splice(0).forEach((client) => client.clear());
  vi.unstubAllGlobals();
});

it.each([
  ["/api", 200],
  ["/api", 401],
  ["https://api.example.test/api/", 200],
  ["https://api.example.test/api/", 401],
] as const)(
  "keeps sign-in available for API base %s with identity status %i",
  async (apiUrl, status) => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation(async (input: RequestInfo | URL) => {
        const url = input instanceof Request ? input.url : String(input);
        if (url.endsWith("/auth/current")) {
          return Response.json(
            status === 401 ? { message: "Unauthorized" } : null,
            {
              status,
            },
          );
        }
        return Response.json({
          providers: [{ id: "sso", label: "Continue with SSO" }],
        });
      }),
    );
    const client = new QueryClient();
    clients.push(client);
    render(
      <Providers
        client={client}
        config={{
          API_URL: apiUrl,
          MAP_STYLE_URL: "opentopomap",
          LOCAL_ADMIN_AUTO_LOGIN: false,
        }}
      >
        <OAuthSignIn />
        <RequireAuth>
          <p>Private activity data</p>
        </RequireAuth>
      </Providers>,
    );
    expect(
      await screen.findByRole("link", { name: "Continue with SSO" }),
    ).toHaveAttribute("href", `${apiUrl.replace(/\/$/, "")}/oauth/sso`);
    expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute(
      "href",
      "/login",
    );
    expect(screen.queryByText("Private activity data")).not.toBeInTheDocument();
  },
);
