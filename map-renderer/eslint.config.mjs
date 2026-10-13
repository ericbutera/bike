import js from "@eslint/js";
import globals from "globals";
import html from "eslint-plugin-html";

export default [
  {
    files: [
      "scripts/**/*.mjs",
      "map-renderer/**/*.mjs",
      "map-renderer/**/*.html",
    ],
    rules: {
      ...js.configs.recommended.rules,
    },
    linterOptions: { reportUnusedDisableDirectives: "error" },
  },
  {
    files: ["map-renderer/**/*.mjs", "scripts/**/*.mjs"],
    languageOptions: { globals: globals.node },
  },
  {
    files: ["map-renderer/**/*.html"],
    plugins: { html },
    languageOptions: { globals: globals.browser },
  },
];
