import { expect, test } from "./helpers/test.mjs";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const mermaidRequire = createRequire(require.resolve("mermaid"));

test("renders Mermaid diagrams and patched KaTeX math in a browser", async ({
  page,
}) => {
  const diagnostics = [];
  page.on("console", (message) => {
    if (["warning", "error"].includes(message.type())) {
      diagnostics.push(message.text());
    }
  });
  page.on("pageerror", (error) => diagnostics.push(error.message));

  await page.setContent(
    '<!DOCTYPE html><html><body><div id="math"></div><div id="diagram"></div></body></html>',
  );
  await page.addScriptTag({
    path: mermaidRequire.resolve("katex/dist/katex.min.js"),
  });
  await page.addScriptTag({
    path: require.resolve("mermaid/dist/mermaid.min.js"),
  });
  await page.evaluate(async () => {
    window.katex.render("x^2 + y^2", document.getElementById("math"), {
      throwOnError: true,
      trust: false,
    });
    window.mermaid.initialize({ startOnLoad: false, securityLevel: "strict" });
    const { svg } = await window.mermaid.render(
      "import-diagram",
      'flowchart LR\n raw_stored["Raw stored"] --> activity_parsed["$$x^2$$"]',
    );
    document.getElementById("diagram").innerHTML = svg;
  });

  await expect(page.locator("#math .katex")).toBeVisible();
  await expect(page.locator("#diagram svg")).toBeVisible();
  await expect(page.locator("#diagram .katex")).toBeVisible();
  expect(diagnostics).toEqual([]);
});
