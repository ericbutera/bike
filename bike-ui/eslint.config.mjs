import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";

export default defineConfig([
  ...nextVitals,
  {
    // React Compiler is not enabled in next.config.ts. Keep its adoption
    // diagnostics visible without making compiler adoption a release gate.
    rules: {
      "react-hooks/set-state-in-effect": "warn",
      "react-hooks/refs": "warn",
      "react-hooks/static-components": "warn",
    },
  },
  globalIgnores([
    ".next/**",
    ".artifacts/**",
    "next-env.d.ts",
    "lib/openapi/react-query/api.d.ts",
  ]),
]);
