"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  createContext,
  useContext,
  useEffect,
  type ComponentType,
  type ReactNode,
} from "react";
import { useRouter } from "next/navigation";
import { config } from "./config";
import { $typedApi, fetchWithCredentials } from "./api";

export type AuthUser = {
  id: string | number;
  email: string;
  name?: string | null;
  verified?: boolean;
  is_admin?: boolean;
};

type AuthConfig = {
  OAuthProviderButtons?: ComponentType<{
    text?: string;
    prefix?: ReactNode;
    unavailable?: ReactNode;
  }>;
};

type AuthContextValue = {
  config: AuthConfig;
};

const AuthContext = createContext<AuthContextValue | null>(null);
const CURRENT_USER_QUERY_KEY = ["get", "/auth/current"] as const;

export function AuthProvider({
  config: authConfig = {},
  children,
}: {
  config?: AuthConfig;
  children: ReactNode;
}) {
  return (
    <AuthContext.Provider
      value={{
        config: authConfig,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuthConfig() {
  const context = useContext(AuthContext);
  if (!context) throw new Error("AuthProvider is required");
  return context.config;
}

function useCurrentUser() {
  const response = $typedApi.useQuery("get", "/auth/current", {
    options: {
      retry: false,
      staleTime: 30 * 60 * 1000,
      gcTime: 60 * 60 * 1000,
    },
  });
  const rawUser = response.isError ? null : response.data;
  return {
    user: rawUser
      ? {
          id: rawUser.pid,
          email: rawUser.email,
          name: rawUser.name,
          verified: rawUser.verified,
          is_admin: rawUser.is_admin,
        }
      : response.isLoading
        ? undefined
        : null,
    isLoading: response.isLoading,
    isError: response.isError,
  };
}

export const authApiClient = {
  useCurrentUser,
  useLogout: () => {
    const queryClient = useQueryClient();
    return useMutation({
      mutationFn: () => fetchWithCredentials(`${config.API_URL}/auth/logout`),
      onSuccess: async () => {
        await queryClient.invalidateQueries({
          queryKey: CURRENT_USER_QUERY_KEY,
        });
      },
    });
  },
};

export function useAuth() {
  const current = useCurrentUser();
  return {
    user: current.user,
    isLoading: current.isLoading,
    isAuthenticated: Boolean(current.user),
    isError: current.isError,
    useCurrentUser: () => current,
    useLogout: authApiClient.useLogout,
  };
}

function AuthCard({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mx-auto w-full max-w-lg rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm">
      <h1 className="text-3xl font-semibold">{title}</h1>
      <div className="mt-6">{children}</div>
    </section>
  );
}

export function OAuthSignIn() {
  const { OAuthProviderButtons } = useAuthConfig();

  return (
    <AuthCard title="Sign in with OAuth/OIDC">
      <p className="text-sm text-base-content/70">
        Continue with your organization&apos;s configured identity provider.
      </p>
      <div className="mt-5">
        {OAuthProviderButtons ? (
          <OAuthProviderButtons
            text="Continue with SSO"
            unavailable={
              <p className="text-sm text-error">
                No OAuth/OIDC provider is currently available.
              </p>
            }
          />
        ) : (
          <p className="text-sm text-error">
            OAuth/OIDC sign-in is not configured.
          </p>
        )}
      </div>
    </AuthCard>
  );
}

export function OAuthCallback() {
  const router = useRouter();
  const current = useCurrentUser();

  useEffect(() => {
    if (!current.isLoading && current.user) {
      router.replace("/");
    }
  }, [current.isLoading, current.user, router]);

  if (current.isError || (!current.isLoading && !current.user)) {
    return (
      <AuthCard title="Sign-in failed">
        <p className="text-sm text-error">
          The identity provider did not complete sign-in. Please try again.
        </p>
      </AuthCard>
    );
  }
  return (
    <AuthCard title="Signing you in">
      <p>Completing OAuth sign-in...</p>
    </AuthCard>
  );
}
