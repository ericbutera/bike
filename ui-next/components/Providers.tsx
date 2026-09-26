"use client";

import { admin, auth, QueryClientProvider } from "@ericbutera/kaleido";
import type { ReactNode } from "react";
import { Toaster } from "react-hot-toast";
import type { AppConfig } from "../lib/config";
import { ConfigProvider } from "../lib/config-context";
import { $api } from "../lib/api";
import { authApiClient, queryClient } from "../lib/kaleido";
import type { components } from "../lib/openapi/react-query/api";
import { UnitPreferencesProvider } from "../lib/unitPreferences";
import AdminNav from "./admin/Nav";
import Navigation from "./Navigation";
import { ReactQueryActivityIndicator } from "./ui/QueryState";

admin.configureAdminLayout({
  SiteNavigation: Navigation,
  AdminNav,
});

export default function Providers({
  config,
  children,
}: {
  config: AppConfig;
  children: ReactNode;
}) {
  const authConfig = {
    passwordAuthEnabled: config.AUTH_PASSWORD_ENABLED,
    registrationEnabled: config.AUTH_REGISTRATION_ENABLED,
    OAuthProviderButtons: createOAuthProviderButtons(config.API_URL),
  };

  return (
    <ConfigProvider initialConfig={config}>
      <QueryClientProvider client={queryClient}>
        <ReactQueryActivityIndicator />
        <auth.AuthProvider client={authApiClient} config={authConfig as any}>
          <UnitPreferencesProvider>{children}</UnitPreferencesProvider>
          <Toaster position="top-right" />
        </auth.AuthProvider>
      </QueryClientProvider>
    </ConfigProvider>
  );
}

type OAuthProviderOption = components["schemas"]["OAuthProviderMetadata"];

function buttonClassName(provider: string): string {
  if (provider === "dev") {
    return "btn btn-secondary w-full";
  }

  return "btn btn-outline w-full";
}

function createOAuthProviderButtons(apiUrl: string) {
  const baseUrl = apiUrl.replace(/\/$/, "");

  return function OAuthProviderButtons({
    text,
    prefix = null,
    unavailable = null,
  }: {
    text?: string;
    prefix?: ReactNode;
    unavailable?: ReactNode;
  }) {
    const visibleProviders = useDiscoveredProviders();

    if (!visibleProviders) {
      return null;
    }

    if (visibleProviders.length === 0) {
      return <>{unavailable}</>;
    }

    return (
      <>
        {prefix}
        <div className="flex w-full flex-col gap-3">
          {visibleProviders.map((provider) => (
            <button
              key={provider.id}
              type="button"
              className={buttonClassName(provider.id)}
              onClick={() => {
                window.location.assign(`${baseUrl}/oauth/${provider.id}`);
              }}
            >
              {visibleProviders.length === 1 && text ? text : provider.label}
            </button>
          ))}
        </div>
      </>
    );
  };
}

function useDiscoveredProviders(): OAuthProviderOption[] | null {
  const response = $api.useQuery("get", "/oauth/providers", {});

  if (response.isLoading) {
    return null;
  }

  if (response.isError || !response.data) {
    return [];
  }

  return normalizeProviders(response.data.providers);
}

function normalizeProviders(
  providers: OAuthProviderOption[],
): OAuthProviderOption[] {
  const seen = new Set<string>();

  return providers
    .map((provider) => ({
      id: provider.id.trim().toLowerCase(),
      label: provider.label?.trim() || `Continue with ${provider.id}`,
    }))
    .filter((provider) => provider.id.length > 0)
    .filter((provider) => {
      if (seen.has(provider.id)) {
        return false;
      }
      seen.add(provider.id);
      return true;
    });
}
