import "../node_modules/maplibre-gl/dist/maplibre-gl.css";
import type { Metadata } from "next";
import Script from "next/script";
import type { ReactNode } from "react";
import Providers from "../components/Providers";
import RuntimeConfigScript from "../components/RuntimeConfigScript";
import { getServerConfig } from "../lib/config";
import "./globals.css";

const siteName = "bike";
const siteDescription = "all vibes bike analytic platform";
const socialPreviewImage = "/social-preview.jpg";

function getMetadataBase() {
  const rawUrl =
    process.env.SITE_URL ??
    process.env.NEXT_PUBLIC_SITE_URL ??
    process.env.FRONTEND_URL ??
    "https://bike.nibelheim.dev";

  try {
    return new URL(rawUrl);
  } catch {
    return new URL("https://bike.nibelheim.dev");
  }
}

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
  metadataBase: getMetadataBase(),
  title: siteName,
  description: siteDescription,
  applicationName: siteName,
  icons: {
    icon: [
      { url: "/favicon.ico" },
      { url: "/favicon-512.png", type: "image/png", sizes: "512x512" },
    ],
    shortcut: [{ url: "/favicon.ico" }],
    apple: [{ url: "/favicon-512.png", type: "image/png", sizes: "512x512" }],
  },
  openGraph: {
    type: "website",
    siteName,
    title: siteName,
    description: siteDescription,
    url: "/",
    images: [
      {
        url: socialPreviewImage,
        width: 1376,
        height: 768,
        alt: siteDescription,
      },
    ],
  },
  twitter: {
    card: "summary_large_image",
    title: siteName,
    description: siteDescription,
    images: [socialPreviewImage],
  },
};

export default function RootLayout({ children }: { children: ReactNode }) {
  const config = getServerConfig();

  return (
    <html lang="en" suppressHydrationWarning>
      <head>
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
