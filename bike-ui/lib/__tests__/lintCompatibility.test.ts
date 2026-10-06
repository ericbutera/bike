// @vitest-environment node

import { ESLint } from "eslint";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("Next lint dependencies", () => {
  const eslint = new ESLint();

  it("accepts a valid component with ESLint 10", async () => {
    const [result] = await eslint.lintText(
      "export default function Page() { return <main><h1>Bike</h1></main>; }",
      { filePath: "app/security-fixture/page.jsx" },
    );
    expect(result.messages).toEqual([]);
  });

  it("keeps React, accessibility, import, and Next rules active", async () => {
    const [result] = await eslint.lintText(
      'export default () => <div>{[1, 2].map(n => <img src="/ride.png" />)}</div>;',
      { filePath: "app/security-fixture/page.jsx" },
    );
    expect(result.messages.map((message) => message.ruleId)).toEqual(
      expect.arrayContaining([
        "react/jsx-key",
        "jsx-a11y/alt-text",
        "import/no-anonymous-default-export",
        "@next/next/no-img-element",
      ]),
    );
  });

  it("resolves configured Next roots without the vulnerable braces parser", () => {
    const nextRequire = createRequire(
      import.meta.resolve("eslint-config-next"),
    );
    const pluginEntry = nextRequire.resolve("@next/eslint-plugin-next");
    const pluginRequire = createRequire(pluginEntry);
    const { getRootDirs } = pluginRequire("./utils/get-root-dirs.js") as {
      getRootDirs(context: {
        cwd: string;
        settings: { next: { rootDir: string | string[] } };
      }): string[];
    };
    // Check the consumer's actual resolved package, rather than a separate copy.
    expect(pluginRequire("fast-glob/package.json").name).toBe("tinyglobby");
    const fixture = mkdtempSync(join(tmpdir(), "bike-lint-roots-"));
    try {
      mkdirSync(join(fixture, "apps", "bike"), { recursive: true });
      mkdirSync(join(fixture, "apps", "maps"));
      writeFileSync(join(fixture, "apps", "README.md"), "fixture");
      const rootDir = join(fixture, "apps", "*");
      const expected = [
        join(fixture, "apps", "bike"),
        join(fixture, "apps", "maps"),
      ];
      expect(
        getRootDirs({ cwd: fixture, settings: { next: { rootDir } } })
          .map((root) => resolve(root))
          .sort(),
      ).toEqual(expected);
      expect(
        getRootDirs({
          cwd: fixture,
          settings: { next: { rootDir: [rootDir] } },
        })
          .map((root) => resolve(root))
          .sort(),
      ).toEqual(expected);
      const nested = "{".repeat(2_000) + "x" + "}".repeat(2_000);
      expect(
        getRootDirs({
          cwd: fixture,
          settings: { next: { rootDir: join(fixture, nested) } },
        }),
      ).toEqual([]);
    } finally {
      rmSync(fixture, { recursive: true, force: true });
    }
  });
});
