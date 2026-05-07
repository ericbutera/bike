"use client";

import { auth } from "@ericbutera/kaleido";
import Link from "next/link";
import { useEffect, useState } from "react";

const STORAGE_KEY = "bike-theme";
const DARK_MEDIA_QUERY = "(prefers-color-scheme: dark)";

type ThemeMode = "light" | "dark";

function getStoredTheme(): ThemeMode | null {
  const storedTheme = window.localStorage.getItem(STORAGE_KEY);

  return storedTheme === "light" || storedTheme === "dark" ? storedTheme : null;
}

function getSystemTheme(): ThemeMode {
  return window.matchMedia(DARK_MEDIA_QUERY).matches ? "dark" : "light";
}

function resolveTheme(): ThemeMode {
  return getStoredTheme() ?? getSystemTheme();
}

function getAppliedTheme(): ThemeMode | null {
  const currentTheme = document.documentElement.getAttribute("data-theme");

  return currentTheme === "light" || currentTheme === "dark"
    ? currentTheme
    : null;
}

function applyTheme(theme: ThemeMode) {
  document.documentElement.setAttribute("data-theme", theme);
  document.documentElement.style.colorScheme = theme;
}

export default function Navigation() {
  const authApi = auth.useAuthApi();
  const { user, isLoading } = authApi.useCurrentUser();
  const logout = authApi.useLogout();
  const [theme, setTheme] = useState<ThemeMode>("light");

  useEffect(() => {
    const nextTheme = getAppliedTheme() ?? resolveTheme();
    setTheme(nextTheme);
    applyTheme(nextTheme);

    const mediaQueryList = window.matchMedia(DARK_MEDIA_QUERY);

    const handleSystemThemeChange = () => {
      if (getStoredTheme()) {
        return;
      }

      const systemTheme = getSystemTheme();
      setTheme(systemTheme);
      applyTheme(systemTheme);
    };

    mediaQueryList.addEventListener("change", handleSystemThemeChange);

    return () => {
      mediaQueryList.removeEventListener("change", handleSystemThemeChange);
    };
  }, []);

  const nextTheme = theme === "dark" ? "light" : "dark";

  return (
    <div className="navbar bg-base-100 shadow-sm">
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <Link href="/" className="btn btn-ghost normal-case text-lg">
            bike
          </Link>
          <div className="hidden items-center gap-2 sm:flex">
            <Link href="/" className="btn btn-ghost btn-sm">
              Activities
            </Link>
            <Link href="/upload" className="btn btn-ghost btn-sm">
              Upload
            </Link>
            <Link href="/segments" className="btn btn-ghost btn-sm">
              Segments
            </Link>
            <Link href="/account" className="btn btn-ghost btn-sm">
              Account
            </Link>
            {user?.is_admin && (
              <Link href="/admin" className="btn btn-ghost btn-sm">
                Admin
              </Link>
            )}
          </div>
        </div>
      </div>
      <div className="flex-none flex items-center gap-2">
        <button
          type="button"
          className="btn btn-ghost btn-sm"
          aria-label={`Switch to ${nextTheme} mode`}
          title={`Switch to ${nextTheme} mode`}
          onClick={() => {
            window.localStorage.setItem(STORAGE_KEY, nextTheme);
            setTheme(nextTheme);
            applyTheme(nextTheme);
          }}
        >
          {theme === "dark" ? "Light mode" : "Dark mode"}
        </button>
        {isLoading ? null : user ? (
          <button
            type="button"
            onClick={() => logout.mutateAsync()}
            disabled={logout.isPending}
            className="btn btn-ghost"
          >
            {logout.isPending ? "Signing out..." : "Sign out"}
          </button>
        ) : (
          <div className="flex items-center gap-2">
            <Link href="/login" className="btn btn-ghost">
              Login
            </Link>
            <Link href="/signup" className="btn btn-primary">
              Sign up
            </Link>
          </div>
        )}
      </div>
    </div>
  );
}
