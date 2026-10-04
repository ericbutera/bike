import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AuthProvider, OAuthSignIn } from "../../lib/auth";

function renderOAuthSignIn() {
  return render(
    <AuthProvider
      config={{
        OAuthProviderButtons: ({ text }) => (
          <button type="button">{text}</button>
        ),
      }}
    >
      <OAuthSignIn />
    </AuthProvider>,
  );
}

describe("OAuth/OIDC auth UI", () => {
  it("renders the configured identity-provider action without password fields", () => {
    renderOAuthSignIn();

    expect(
      screen.getByRole("heading", { name: "Sign in with OAuth/OIDC" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Continue with SSO" }),
    ).toBeInTheDocument();
    expect(screen.queryByLabelText(/password/i)).not.toBeInTheDocument();
  });
});
