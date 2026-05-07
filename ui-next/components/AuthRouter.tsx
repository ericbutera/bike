"use client";

import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useEffect, useRef, type ReactNode } from "react";
import { MemoryRouter, useLocation } from "react-router-dom";

type AuthRouteState = {
  email?: string;
};

function getEmailFromState(state: unknown) {
  if (typeof state !== "object" || state === null || !("email" in state)) {
    return null;
  }

  const email = (state as AuthRouteState).email;
  return typeof email === "string" ? email : null;
}

export function buildSyncedTarget(
  pathname: string,
  search: string,
  state: unknown,
) {
  const params = new URLSearchParams(search);
  const email = getEmailFromState(state);

  if (email && !params.has("email")) {
    params.set("email", email);
  }

  const query = params.toString();
  return query ? `${pathname}?${query}` : pathname;
}

export function buildInitialRouteState(pathname: string, email: string | null) {
  if (email !== null) {
    return { email } satisfies AuthRouteState;
  }

  if (pathname === "/confirm-email") {
    return { email: "" } satisfies AuthRouteState;
  }

  return null;
}

function RouterSync() {
  const nextRouter = useRouter();
  const location = useLocation();
  const lastSyncedRef = useRef<string | null>(null);

  useEffect(() => {
    const target = buildSyncedTarget(
      location.pathname,
      location.search,
      location.state,
    );
    if (target === lastSyncedRef.current) {
      return;
    }
    lastSyncedRef.current = target;
    nextRouter.replace(target);
  }, [location.pathname, location.search, nextRouter]);

  return null;
}

export default function AuthRouter({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const query = searchParams.toString();
  const entryState = buildInitialRouteState(
    pathname,
    searchParams.get("email"),
  );

  return (
    <MemoryRouter
      initialEntries={[
        {
          pathname,
          search: query ? `?${query}` : "",
          state: entryState,
        },
      ]}
    >
      <RouterSync />
      {children}
    </MemoryRouter>
  );
}
