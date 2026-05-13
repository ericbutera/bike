import "leaflet/dist/leaflet.css";
import type { Metadata } from "next";
import Script from "next/script";
import type { ReactNode } from "react";
import Providers from "../components/Providers";
import RuntimeConfigScript from "../components/RuntimeConfigScript";
import { getServerConfig } from "../lib/config";
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

export const dynamic = "force-dynamic";

export const metadata: Metadata = {
  title: "bike",
  description: "Next.js frontend scaffold",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  const config = getServerConfig();

  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <link rel="icon" href="/favicon.ico" />
        <link rel="alternate icon" href="/favicon-512.png" />
        <Script id="bike-theme-init" strategy="beforeInteractive">
          {themeScript}
        </Script>
      </head>
      <body className="bg-base-200 text-base-content antialiased">
        <RuntimeConfigScript config={config} />
        <Providers config={config}>{children}</Providers>
      </body>
    </html>
  );
}
