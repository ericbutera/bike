import { spawn, execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const child = spawn(
  process.execPath,
  [join(root, "run.mjs"), "--experiment", "experiments/EXP001-smoke.json"],
  { cwd: root, stdio: ["ignore", "pipe", "pipe"] },
);
let stdout = "",
  stderr = "",
  interrupted = false;
child.stdout.on("data", (chunk) => {
  stdout += chunk;
  if (!interrupted && stdout.includes("Restore/prepare history-1x/V0")) {
    interrupted = true;
    setTimeout(() => child.kill("SIGINT"), 500);
  }
});
child.stderr.on("data", (chunk) => (stderr += chunk));
const code = await new Promise((resolve, reject) => {
  child.once("close", resolve);
  child.once("error", reject);
});
const id = stdout.match(/Run ([^;]+);/)?.[1];
if (!id) throw new Error(`No run identity; Docker startup failed. ${stderr}`);
const output = join(root, "results", id);
const report = JSON.parse(await readFile(join(output, "report.json")));
const identity = JSON.parse(await readFile(join(output, "run.json")));
const resources = {};
for (const [kind, args] of [
  ["containers", ["ps", "-a", "--format", "{{.Names}}"]],
  ["volumes", ["volume", "ls", "--format", "{{.Name}}"]],
  ["networks", ["network", "ls", "--format", "{{.Name}}"]],
])
  resources[kind] = execFileSync(
    "docker",
    [
      ...args,
      "--filter",
      `label=com.docker.compose.project=${identity.project}`,
    ],
    { encoding: "utf8" },
  ).trim();
const runtime = report.environment.database_runtime_limits;
const limits = report.environment.limits;
const result = {
  id,
  project: identity.project,
  exit_code: code,
  interrupted,
  outcome: report.outcome,
  cleanup: report.cleanup,
  durable_identity_matches: identity.project === report.project,
  runtime_limits: runtime,
  resources,
};
result.passed =
  code === 1 &&
  interrupted &&
  report.outcome === "interrupted" &&
  report.cleanup === "passed" &&
  result.durable_identity_matches &&
  Object.values(resources).every((value) => value === "") &&
  runtime?.memory_bytes === limits.postgres_memory_bytes &&
  runtime.cpu_nanos === limits.postgres_cpus * 1e9;
await writeFile(join(output, "cleanup-check.log"), stdout + stderr);
await writeFile(
  join(output, "cleanup-check.json"),
  JSON.stringify(result, null, 2) + "\n",
);
console.log(JSON.stringify(result, null, 2));
if (!result.passed)
  throw new Error(`Cleanup/recovery check failed; see ${output}`);
