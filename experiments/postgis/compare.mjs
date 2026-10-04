import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { compatibility, digest } from "./harness.mjs";

const args = process.argv.slice(2);
if (args.length !== 2)
  throw new Error(
    "Usage: mise run experiment:compare -- results/<before> results/<after>",
  );
const runs = await Promise.all(
  args.map(async (path) =>
    JSON.parse(await readFile(resolve(path, "report.json"), "utf8")),
  ),
);
if (
  runs.some(
    (run) =>
      run.outcome !== "passed" ||
      run.cleanup !== "passed" ||
      run.correctness.some((c) => !c.passed),
  )
)
  throw new Error("Only completed, correct, cleaned-up runs can be compared");
if (digest(compatibility(runs[0])) !== digest(compatibility(runs[1])))
  throw new Error(
    "Fixture, environment, sample settings or workloads differ; this is not a controlled rerun",
  );
const changes = [
  ...new Set(runs.flatMap((run) => Object.keys(run.environment.source_files))),
].filter(
  (file) =>
    runs[0].environment.source_files[file] !==
    runs[1].environment.source_files[file],
);
const lines = [
  `# ${runs[0].id} → ${runs[1].id}`,
  "",
  `Source files changed: ${changes.join(", ") || "none"}.`,
  "",
  "Equal outputs verified in both runs. Small sample changes describe variation, not an adoption decision.",
  "",
  "| Corpus | Case | Arm | Before ms | After ms | Change % | Rust CPU change % | PG CPU change % |",
  "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |",
];
const percent = (a, b) => (a === 0 ? "—" : ((b / a - 1) * 100).toFixed(1));
for (const after of runs[1].summaries) {
  const before = runs[0].summaries.find((s) => s.key === after.key);
  if (!before) throw new Error(`Missing baseline ${after.key}`);
  if (before.fingerprint !== after.fingerprint)
    throw new Error(
      `Output changed between runs for ${after.key}; use a separately reviewed behavior experiment`,
    );
  lines.push(
    `| ${after.corpus} | ${after.case} | ${after.arm} | ${before.median_ms.toFixed(2)} | ${after.median_ms.toFixed(2)} | ${percent(before.median_ms, after.median_ms)} | ${percent(before.rust_cpu_ms, after.rust_cpu_ms)} | ${percent(before.pg_cpu_ms, after.pg_cpu_ms)} |`,
  );
}
const path = resolve(args[1], "comparison.md");
await writeFile(path, lines.join("\n") + "\n");
console.log(path);
