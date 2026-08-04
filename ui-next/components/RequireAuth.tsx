"use client";

import { auth } from "@ericbutera/kaleido";
import { createContext, useContext, type ReactNode } from "react";
import AuthRequiredCard from "./AuthRequiredCard";

type AuthenticatedUser = {
  id?: number;
  name?: string | null;
  email?: string | null;
};

const AuthenticatedUserContext = createContext<AuthenticatedUser | null>(null);

function AuthLoadingCard() {
  return (
    <section className="card bg-base-100 shadow-xl">
      <div className="card-body items-center py-10">
        <span className="loading loading-spinner loading-md" />
      </div>
    </section>
  );
}

export default function RequireAuth({ children }: { children: ReactNode }) {
  const authApi = auth.useAuthApi();
  const { user, isLoading } = authApi.useCurrentUser();

  if (isLoading) {
    return <AuthLoadingCard />;
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

export { useAuthenticatedUser };
