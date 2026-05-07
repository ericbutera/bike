"use client";

import { admin, auth, QueryClientProvider } from "@ericbutera/kaleido";
import type { ReactNode } from "react";
import { Toaster } from "react-hot-toast";
import AdminNav from "./admin/Nav";
import Navigation from "./Navigation";
import { authApiClient, queryClient } from "../lib/kaleido";

admin.configureAdminLayout({
  SiteNavigation: Navigation,
  AdminNav,
});

export default function Providers({ children }: { children: ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>
      <auth.AuthProvider client={authApiClient}>
        {children}
        <Toaster position="top-right" />
      </auth.AuthProvider>
    </QueryClientProvider>
  );
}
