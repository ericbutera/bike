import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const projectRoot = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  plugins: [react()],
  test: {
    exclude: ["**/node_modules/**", "**/.git/**", "tests/e2e/**"],
    environment: "happy-dom",
    globals: true,
    setupFiles: ["./vitest.setup.ts"],
    coverage: {
      provider: "v8",
      reportsDirectory: fileURLToPath(
        new URL("../.artifacts/coverage/bike-ui", import.meta.url),
      ),
      include: ["{app,components,lib}/**/*.{ts,tsx}"],
      exclude: ["**/*.d.ts", "**/*.{test,spec}.{ts,tsx}", "**/__tests__/**"],
      reporter: [
        "text",
        ["html", { subdir: "html" }],
        [
          "lcovonly",
          { projectRoot: fileURLToPath(new URL("..", import.meta.url)) },
        ],
        "json-summary",
      ],
    },
    alias: {
      "@": projectRoot,
    },
  },
  resolve: {
    alias: {
      "@": projectRoot,
    },
  },
});
