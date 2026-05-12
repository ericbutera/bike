"use client";

import { admin, auth, QueryClientProvider } from "@ericbutera/kaleido";
import type { ReactNode } from "react";
import { Toaster } from "react-hot-toast";
import type { AppConfig } from "../lib/config";
import { ConfigProvider } from "../lib/config-context";
import { authApiClient, queryClient } from "../lib/kaleido";
import { UnitPreferencesProvider } from "../lib/unitPreferences";
import AdminNav from "./admin/Nav";
import Navigation from "./Navigation";

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
  return (
    <ConfigProvider initialConfig={config}>
      <QueryClientProvider client={queryClient}>
        <auth.AuthProvider client={authApiClient}>
          <UnitPreferencesProvider>{children}</UnitPreferencesProvider>
          <Toaster position="top-right" />
        </auth.AuthProvider>
      </QueryClientProvider>
    </ConfigProvider>
  );
}
