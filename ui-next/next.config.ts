import type { NextConfig } from "next";
import fs from "node:fs";
import path from "node:path";

// When pointing Turbopack/webpack at the raw kaleido source, that source
// resolves its own imports (react, react-query, react-router-dom, etc.) from
// kaleido's node_modules, producing a second instance of those singletons.
// Force them all to the app's copies so there's only one instance.

// Turbopack: project-relative paths (must not start with ../)
const singletonAliasesTurbopack = {
  react: "./node_modules/react",
  "react-dom": "./node_modules/react-dom",
  "@tanstack/react-query": "./node_modules/@tanstack/react-query",
  "react-router-dom": "./node_modules/react-router-dom",
};

// Webpack: absolute paths
const singletonAliasesWebpack = (appRoot: string) => ({
  react: path.join(appRoot, "node_modules/react"),
  "react-dom": path.join(appRoot, "node_modules/react-dom"),
  "@tanstack/react-query": path.join(
    appRoot,
    "node_modules/@tanstack/react-query",
  ),
  "react-router-dom": path.join(appRoot, "node_modules/react-router-dom"),
});

const localKaleidoEntry = path.resolve(
  __dirname,
  "../../kaleido/typescript/packages/kaleido/src/index.ts",
);

const useLocalKaleido =
  process.env.NODE_ENV !== "production" && fs.existsSync(localKaleidoEntry);

const turbopackAlias = (() => {
  if (!useLocalKaleido) return null;
  const rel = path
    .relative(__dirname, localKaleidoEntry)
    .split(path.sep)
    .join("/");
  if (rel.startsWith("../")) return null;
  return `./${rel}`;
})();

const nextConfig: NextConfig = {
  output: "standalone",
  transpilePackages: ["@ericbutera/kaleido"],
  experimental: {
    externalDir: true,
  },
  turbopack: {
    resolveAlias:
      useLocalKaleido && turbopackAlias
        ? {
            "@ericbutera/kaleido": turbopackAlias,
            ...singletonAliasesTurbopack,
          }
        : undefined,
  },
  webpack: (config) => {
    if (useLocalKaleido) {
      config.resolve.alias = {
        ...(config.resolve.alias ?? {}),
        "@ericbutera/kaleido": localKaleidoEntry,
        ...singletonAliasesWebpack(__dirname),
      };
    }
    return config;
  },
};

export default nextConfig;
