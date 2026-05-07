import "leaflet/dist/leaflet.css";
import type { Metadata } from "next";
import Script from "next/script";
import type { ReactNode } from "react";
import Providers from "../components/Providers";
import "./globals.css";

const themeScript = `
(() => {
  try {
    const storageKey = "bike-theme";
    const storedTheme = window.localStorage.getItem(storageKey);
    const theme =
      storedTheme === "light" || storedTheme === "dark"
        ? storedTheme
        : window.matchMedia("(prefers-color-scheme: dark)").matches
          ? "dark"
          : "light";

    document.documentElement.setAttribute("data-theme", theme);
    document.documentElement.style.colorScheme = theme;
  } catch (_error) {
    document.documentElement.setAttribute("data-theme", "light");
    document.documentElement.style.colorScheme = "light";
  }
})();
`;

export const metadata: Metadata = {
  title: "bike",
  description: "Next.js frontend scaffold",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <Script id="bike-theme-init" strategy="beforeInteractive">
          {themeScript}
        </Script>
      </head>
      <body className="bg-base-200 text-base-content antialiased">
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
