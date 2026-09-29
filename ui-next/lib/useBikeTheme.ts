"use client";

import { useEffect, useState } from "react";

export type BikeTheme = "light" | "dark";

function appliedTheme(): BikeTheme {
  return document.documentElement.getAttribute("data-theme") === "dark"
    ? "dark"
    : "light";
}

export function useBikeTheme(): BikeTheme {
  const [theme, setTheme] = useState<BikeTheme>("light");

  useEffect(() => {
    const update = () => setTheme(appliedTheme());
    update();
    const observer = new MutationObserver(update);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, []);

  return theme;
}
