import { createRequire } from "node:module";
import mermaid from "mermaid";
import { expect, it } from "vitest";

it("keeps Mermaid math rendering compatible with patched KaTeX", async () => {
  const mermaidRequire = createRequire(import.meta.resolve("mermaid"));
  const katex = mermaidRequire("katex") as {
    renderToString(
      expression: string,
      options: { throwOnError: boolean; trust: boolean },
    ): string;
  };
  const result = katex.renderToString("x^2 + y^2", {
    throwOnError: true,
    trust: false,
  });
  expect(result).toContain('class="katex"');
  mermaid.initialize({ startOnLoad: false, securityLevel: "strict" });
  expect(
    await mermaid.parse(
      'flowchart LR\n raw_stored["Raw stored"] --> activity_parsed["$$x^2$$"]',
    ),
  ).toMatchObject({ diagramType: "flowchart-v2" });
});
