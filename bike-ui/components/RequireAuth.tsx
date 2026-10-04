"use client";

import { createContext, useContext, type ReactNode } from "react";
import { useAuth } from "../lib/auth";
import AuthRequiredCard from "./AuthRequiredCard";
import { LoadingCard } from "./ui/QueryState";

type AuthenticatedUser = {
  id?: number | string;
  name?: string | null;
  email?: string | null;
  is_admin?: boolean;
};

const AuthenticatedUserContext = createContext<AuthenticatedUser | null>(null);

export default function RequireAuth({ children }: { children: ReactNode }) {
  const { user, isLoading } = useAuth();

  if (isLoading) {
    return <LoadingCard />;
  }

  if (!user) {
    return <AuthRequiredCard />;
  }

  return (
    <AuthenticatedUserContext.Provider value={user as AuthenticatedUser}>
      {children}
    </AuthenticatedUserContext.Provider>
  );
}

function useAuthenticatedUser() {
  const user = useContext(AuthenticatedUserContext);

  if (!user) {
    throw new Error("useAuthenticatedUser must be used inside RequireAuth.");
  }

  return user;
}

function getAuthenticatedUserId(user: {
  id?: number | string | null;
  pid?: number | string | null;
}) {
  const value = user.id ?? user.pid;
  const numericValue =
    typeof value === "number" ? value : value != null ? Number(value) : NaN;
  return Number.isFinite(numericValue) ? numericValue : null;
}

export { getAuthenticatedUserId, useAuthenticatedUser };
