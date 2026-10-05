"use client";

import { useEffect, useSyncExternalStore } from "react";

const STORAGE_KEY = "bike-theme";
const DARK_MEDIA_QUERY = "(prefers-color-scheme: dark)";
const THEME_CHANGE_EVENT = "bike:theme-change";

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

function getThemeSnapshot(): ThemeMode {
  return getAppliedTheme() ?? resolveTheme();
}

function subscribeToTheme(onChange: () => void) {
  const mediaQueryList = window.matchMedia(DARK_MEDIA_QUERY);
  const handleChange = () => {
    applyTheme(resolveTheme());
    onChange();
  };

  mediaQueryList.addEventListener("change", handleChange);
  window.addEventListener("storage", handleChange);
  window.addEventListener(THEME_CHANGE_EVENT, handleChange);

  return () => {
    mediaQueryList.removeEventListener("change", handleChange);
    window.removeEventListener("storage", handleChange);
    window.removeEventListener(THEME_CHANGE_EVENT, handleChange);
  };
}

export default function ThemeToggle() {
  const theme = useSyncExternalStore(
    subscribeToTheme,
    getThemeSnapshot,
    () => "light",
  );

  useEffect(() => {
    applyTheme(getAppliedTheme() ?? resolveTheme());
  }, []);

  const nextTheme = theme === "dark" ? "light" : "dark";

  return (
    <label className="flex w-full cursor-pointer items-center justify-between gap-3">
      <span>Theme: {theme === "dark" ? "Dark mode" : "Light mode"}</span>
      <input
        type="checkbox"
        value="dark"
        className="toggle toggle-sm theme-controller"
        checked={theme === "dark"}
        aria-label={`Switch to ${nextTheme} mode`}
        title={`Switch to ${nextTheme} mode`}
        onChange={(event) => {
          const selectedTheme: ThemeMode = event.target.checked
            ? "dark"
            : "light";
          window.localStorage.setItem(STORAGE_KEY, selectedTheme);
          applyTheme(selectedTheme);
          window.dispatchEvent(new Event(THEME_CHANGE_EVENT));
        }}
      />
    </label>
  );
}
