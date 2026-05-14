"use client";

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

export default function ThemeToggle() {
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
    <button
      type="button"
      className="w-full text-left"
      aria-label={`Switch to ${nextTheme} mode`}
      title={`Switch to ${nextTheme} mode`}
      onClick={() => {
        window.localStorage.setItem(STORAGE_KEY, nextTheme);
        setTheme(nextTheme);
        applyTheme(nextTheme);
      }}
    >
      Theme: {theme === "dark" ? "Light mode" : "Dark mode"}
    </button>
  );
}
