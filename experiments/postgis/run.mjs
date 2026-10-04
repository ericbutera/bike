import { spawn, execFileSync } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { createReadStream } from "node:fs";
import {
  mkdir,
  readFile,
  writeFile,
  appendFile,
  readdir,
} from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { pipeline } from "node:stream/promises";
import { once } from "node:events";
import { createGunzip } from "node:zlib";
import { fileURLToPath } from "node:url";
import {
  configuration,
  workloadCases,
  summarize,
  correctness,
  reportMarkdown,
  digest,
  discoverySQL,
  imageIdentity,
  resolvedLimits,
} from "./harness.mjs";

const root = dirname(fileURLToPath(import.meta.url));
const repository = resolve(root, "../..");
const option = (name, fallback) => {
  const index = process.argv.indexOf(name);
  return index < 0 ? fallback : process.argv[index + 1];
};
const config = await configuration(
  resolve(root, option("--experiment", "experiments/EXP001.json")),
);
const id = `${new Date().toISOString().replace(/[-:.]/g, "")}-${config.id.toLowerCase()}-${randomBytes(3).toString("hex")}`;
const output = join(root, "results", id);
const project = `bike-postgis-eval-${randomBytes(6).toString("hex")}`;
await mkdir(output, { recursive: true });
for (const folder of ["input", "batches", "plans", "setup"])
  await mkdir(join(output, folder));
const imageTag = project.slice("bike-postgis-eval-".length);
const env = {
  ...process.env,
  POSTGIS_RUN_DIR: output,
  POSTGIS_IMAGE_TAG: imageTag,
};
const compose = [
  "compose",
  "--project-name",
  project,
  "--file",
  join(root, "compose.yaml"),
];
const controller = new AbortController();
for (const signal of ["SIGINT", "SIGTERM"])
  process.once(signal, () =>
    controller.abort(new Error(`Interrupted by ${signal}`)),
  );
const record = {
  schema_version: 1,
  id,
  project,
  started_at: new Date().toISOString(),
  config,
  environment: {},
  fixtures: [],
  workloads: {},
  batches: [],
  storage: [],
  summaries: [],
  correctness: [],
  outcome: "running",
};
// This survives an uncatchable process/host failure before the final report.
await writeFile(
  join(output, "run.json"),
  JSON.stringify(
    {
      id,
      project,
      image_tag: imageTag,
      output,
      started_at: record.started_at,
      experiment: config.id,
      revision: config.revision,
    },
    null,
    2,
  ) + "\n",
);
let booted = false;

function command(args, { quiet = false, cleanup = false } = {}) {
  return new Promise((fulfill, reject) => {
    const child = spawn("docker", args, {
      cwd: root,
      env,
      signal: cleanup ? undefined : controller.signal,
      stdio: ["ignore", quiet ? "pipe" : "inherit", "inherit"],
    });
    let stdout = "";
    child.stdout?.on("data", (chunk) => (stdout += chunk));
    child.once("error", reject);
    child.once("close", (code) =>
      code === 0
        ? fulfill(stdout.trim())
        : reject(
            new Error(`docker ${args.slice(0, 3).join(" ")} exited ${code}`),
          ),
    );
  });
}
const dc = (args, options) => command([...compose, ...args], options);
async function sql(sql, database = "experiment") {
  const child = spawn(
    "docker",
    [
      ...compose,
      "exec",
      "-T",
      "database",
      "psql",
      "-XqAt",
      "-U",
      "postgres",
      "-d",
      database,
      "-v",
      "ON_ERROR_STOP=1",
    ],
    {
      cwd: root,
      env,
      signal: controller.signal,
      stdio: ["pipe", "pipe", "inherit"],
    },
  );
  const exited = once(child, "close");
  let output = "";
  child.stdout.on("data", (chunk) => (output += chunk));
  child.stdin.end(sql);
  const [code] = await exited;
  if (code !== 0) throw new Error("Experiment SQL failed");
  return output.trim();
}
async function restore(corpus, arm) {
  const database = `${corpus.replace("-", "_")}_${arm.toLowerCase()}`;
  await dc(["exec", "-T", "database", "createdb", "-U", "postgres", database]);
  const child = spawn(
    "docker",
    [
      ...compose,
      "exec",
      "-T",
      "database",
      "psql",
      "-Xq",
      "-U",
      "postgres",
      "-d",
      database,
      "-v",
      "ON_ERROR_STOP=1",
    ],
    {
      cwd: root,
      env,
      signal: controller.signal,
      stdio: ["pipe", "ignore", "inherit"],
    },
  );
  const exited = once(child, "close");
  const [, [code]] = await Promise.all([
    pipeline(
      createReadStream(join(root, "fixtures", `${corpus}.sql.gz`)),
      createGunzip(),
      child.stdin,
    ),
    exited,
  ]);
  if (code !== 0) throw new Error(`Restore failed for ${corpus}/${arm}`);
  return database;
}
async function fingerprint(file) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest("hex");
}
async function sourceFingerprint() {
  const paths = [
    "harness.mjs",
    "run.mjs",
    "compare.mjs",
    "compose.yaml",
    "database.Dockerfile",
    "database.Dockerfile.dockerignore",
    "probe.Dockerfile",
    "probe.Dockerfile.dockerignore",
    "probe/Cargo.toml",
    "probe/Cargo.lock",
    "probe/build.rs",
    "ideas.json",
  ];
  for (const file of (await readdir(join(root, "probe/src"))).sort())
    paths.push(`probe/src/${file}`);
  const files = {};
  const snapshot = {};
  for (const file of paths) {
    const content = await readFile(join(root, file), "utf8");
    files[file] = createHash("sha256").update(content).digest("hex");
    snapshot[`experiments/postgis/${file}`] = content;
  }
  for (const file of [
    "activity_data.rs",
    "activity_sport.rs",
    "segment_support.rs",
  ]) {
    const content = await readFile(
      join(repository, "bike-rs/bike-core/src", file),
      "utf8",
    );
    files[`bike-rs/${file}`] = createHash("sha256")
      .update(content)
      .digest("hex");
    snapshot[`bike-rs/bike-core/src/${file}`] = content;
  }
  return { files, sha256: digest(files), snapshot };
}
try {
  const ideas = JSON.parse(await readFile(join(root, "ideas.json"), "utf8"));
  if (config.ideas.some((id) => !ideas.ideas.some((idea) => idea.id === id)))
    throw new Error("Experiment references unknown idea");
  record.ideas = ideas.ideas.filter((i) => config.ideas.includes(i.id));
  const sources = await sourceFingerprint();
  record.environment.source_files = sources.files;
  record.environment.source_sha256 = sources.sha256;
  await writeFile(
    join(output, "source-snapshot.json"),
    JSON.stringify(sources.snapshot, null, 2) + "\n",
  );
  record.environment.revision = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repository,
    encoding: "utf8",
  }).trim();
  record.environment.dirty =
    execFileSync("git", ["status", "--porcelain"], {
      cwd: repository,
      encoding: "utf8",
    }).trim().length > 0;
  record.environment.docker = JSON.parse(
    await command(["info", "--format", "{{json .}}"], { quiet: true }),
  );
  // Store relevant host properties rather than complete daemon configuration.
  const docker = record.environment.docker;
  record.environment.docker = Object.fromEntries(
    [
      "Architecture",
      "NCPU",
      "MemTotal",
      "KernelVersion",
      "OperatingSystem",
      "Driver",
      "ServerVersion",
    ].map((key) => [key, docker[key]]),
  );
  const resolved = JSON.parse(
    await dc(["config", "--format", "json"], { quiet: true }),
  );
  record.environment.limits = resolvedLimits(resolved.services);
  record.environment.compose = await command(
    ["compose", "version", "--short"],
    { quiet: true },
  );
  record.environment.context = await command(["context", "show"], {
    quiet: true,
  });
  await writeFile(
    join(output, "experiment.json"),
    JSON.stringify(config, null, 2) + "\n",
  );
  await writeFile(
    join(output, "ideas.json"),
    JSON.stringify(record.ideas, null, 2) + "\n",
  );
  const selector = JSON.parse(
    await readFile(join(root, "fixtures/private-cases.json"), "utf8"),
  );
  for (const corpus of config.corpora) {
    const manifest = JSON.parse(
      await readFile(join(root, "fixtures", `${corpus}.manifest.json`), "utf8"),
    );
    if (
      (await fingerprint(join(root, "fixtures", `${corpus}.sql.gz`))) !==
      manifest.dump.sha256
    )
      throw new Error(`Fixture checksum mismatch: ${corpus}`);
    record.fixtures.push({
      corpus,
      sha256: manifest.dump.sha256,
      activities: manifest.activity_count,
      route_points: manifest.total_route_points,
    });
  }
  console.log(`Run ${id}; project ${project}; ${config.corpora.join(", ")}`);
  await dc(["build"]);
  for (const [key, image] of [
    ["database_image", `bike-postgis-eval-database:${imageTag}`],
    ["probe_image", `bike-postgis-eval-probe:${imageTag}`],
  ]) {
    const info = JSON.parse(
      await command(["image", "inspect", image], { quiet: true }),
    )[0];
    record.environment[key] = {
      id: info.Id,
      architecture: info.Architecture,
      repo_digests: info.RepoDigests,
      content_sha256: imageIdentity(info),
      size_bytes: info.Size,
    };
  }
  const metadata = JSON.parse(
    await command(
      [
        "run",
        "--rm",
        "--network",
        "none",
        `bike-postgis-eval-probe:${imageTag}`,
        "metadata",
      ],
      { quiet: true },
    ),
  );
  record.environment.probe_toolchain = `${metadata.rustc}; ${metadata.target}`;
  await writeFile(
    join(output, "environment.json"),
    JSON.stringify(record.environment, null, 2) + "\n",
  );
  booted = true;
  await dc(["up", "--detach", "--wait", "database"]);
  const container = await dc(["ps", "--quiet", "database"], { quiet: true });
  const runtime = JSON.parse(
    await command(["inspect", container], { quiet: true }),
  )[0].HostConfig;
  record.environment.database_runtime_limits = {
    cpu_nanos: runtime.NanoCpus,
    memory_bytes: runtime.Memory,
    shared_memory_bytes: runtime.ShmSize,
  };
  if (
    runtime.Memory !== record.environment.limits.postgres_memory_bytes ||
    runtime.NanoCpus !== record.environment.limits.postgres_cpus * 1e9
  )
    throw new Error(
      "Database runtime limits differ from resolved Compose configuration",
    );
  record.environment.postgres_settings = JSON.parse(
    await sql(
      "SELECT jsonb_object_agg(name,setting) FROM pg_settings WHERE name IN ('shared_buffers','work_mem','jit','max_parallel_workers_per_gather','autovacuum','track_io_timing','max_connections','fsync','full_page_writes','synchronous_commit')",
    ),
  );
  await writeFile(
    join(output, "environment.json"),
    JSON.stringify(record.environment, null, 2) + "\n",
  );
  for (const corpus of config.corpora) {
    const manifest = JSON.parse(
      await readFile(join(root, "fixtures", `${corpus}.manifest.json`), "utf8"),
    );
    const databases = {};
    for (const arm of config.arms) {
      console.log(`Restore/prepare ${corpus}/${arm}`);
      databases[arm] = await restore(corpus, arm);
      const url = `host=database user=postgres password=experiment-only dbname=${databases[arm]}`;
      const path = `setup/${corpus}-${arm}.json`;
      await dc([
        "run",
        "--rm",
        "--no-deps",
        "-T",
        "-e",
        `DATABASE_URL=${url}`,
        "probe",
        "prepare",
        arm,
        `/output/${path}`,
      ]);
      const storage = JSON.parse(await readFile(join(output, path), "utf8"));
      for (const [actual, expected] of [
        [storage.activities, manifest.activity_count],
        [storage.segments, manifest.segment_count],
        [storage.efforts, manifest.effort_count],
        [storage.route_points, manifest.total_route_points],
      ])
        if (actual !== expected)
          throw new Error(`Restored count mismatch: ${corpus}/${arm}`);
      record.storage.push({ ...storage, corpus });
    }
    const other = [...selector.regions].sort(
      (a, b) => b.activities - a.activities,
    )[1].center;
    const discovery = JSON.parse(await sql(discoverySQL(other), databases.V0));
    const cases = workloadCases(config, selector, manifest, discovery);
    record.workloads[corpus] = cases;
    for (const item of cases)
      await writeFile(
        join(output, "input", `${corpus}-${item.name}.json`),
        JSON.stringify(item, null, 2),
      );
    for (let batch = 0; batch < config.batches; batch++) {
      const arms = batch % 2 ? [...config.arms].reverse() : config.arms;
      for (const item of cases)
        for (const arm of arms) {
          console.log(
            `${corpus} batch ${batch + 1}/${config.batches}: ${arm} ${item.name}`,
          );
          const path = `batches/${corpus}-${item.name}-${arm}-${batch}.json`;
          const url = `host=database user=postgres password=experiment-only dbname=${databases[arm]}`;
          await dc([
            "run",
            "--rm",
            "--no-deps",
            "-T",
            "-e",
            `DATABASE_URL=${url}`,
            "probe",
            "run",
            arm,
            `/output/input/${corpus}-${item.name}.json`,
            `/output/${path}`,
          ]);
          const result = {
            ...JSON.parse(await readFile(join(output, path), "utf8")),
            corpus,
            batch,
          };
          record.batches.push(result);
          for (const observation of result.observations)
            await appendFile(
              join(output, "observations.jsonl"),
              JSON.stringify({
                ...observation,
                corpus,
                batch,
                arm,
                case: item.name,
              }) + "\n",
            );
          await writeFile(
            join(
              output,
              "plans",
              `${corpus}-${item.name}-${arm}-${batch}.json`,
            ),
            await readFile(join(output, `${path}.plans.json`)),
          );
        }
    }
  }
  record.summaries = summarize(record.batches);
  if ((await sourceFingerprint()).sha256 !== record.environment.source_sha256)
    throw new Error(
      "Experiment source changed during the run; results are not a controlled snapshot",
    );
  record.correctness = correctness(record.summaries);
  if (record.correctness.some((c) => !c.passed))
    throw new Error(
      "Cross-arm output equality failed; performance ranking is invalid",
    );
  record.outcome = "passed";
} catch (error) {
  record.outcome = controller.signal.aborted ? "interrupted" : "failed";
  record.error = error.message;
  process.exitCode = 1;
} finally {
  if (booted) {
    try {
      await dc(["down", "--volumes", "--remove-orphans", "--timeout", "10"], {
        cleanup: true,
      });
      record.cleanup = "passed";
    } catch (error) {
      record.cleanup = "failed";
      record.error = `${record.error ?? ""} Cleanup: ${error.message}`;
      record.outcome = "failed";
      process.exitCode = 1;
    }
  } else record.cleanup = "not_started";
  record.finished_at = new Date().toISOString();
  record.summaries = summarize(record.batches);
  record.correctness = correctness(record.summaries);
  await writeFile(
    join(output, "storage.json"),
    JSON.stringify(record.storage, null, 2) + "\n",
  );
  await writeFile(
    join(output, "workloads.json"),
    JSON.stringify(record.workloads, null, 2) + "\n",
  );
  await writeFile(
    join(output, "correctness.json"),
    JSON.stringify(record.correctness, null, 2) + "\n",
  );
  await writeFile(
    join(output, "report.json"),
    JSON.stringify(record, null, 2) + "\n",
  );
  // Early failures may not have reached image discovery.
  if (record.environment.database_image && record.environment.probe_image)
    await writeFile(join(output, "report.md"), reportMarkdown(record));
  else
    await writeFile(
      join(output, "report.md"),
      `# ${id}\n\nOutcome: ${record.outcome}.\n\n${record.error ?? ""}\n`,
    );
  await appendFile(
    join(root, "results/index.jsonl"),
    JSON.stringify({
      id,
      experiment: config.id,
      revision: config.revision,
      ideas: config.ideas,
      outcome: record.outcome,
      cleanup: record.cleanup,
      report: `${id}/report.md`,
      source_sha256: record.environment.source_sha256,
    }) + "\n",
  );
  console.log(
    `Report: ${join(output, "report.md")}; outcome ${record.outcome}; cleanup ${record.cleanup}`,
  );
}
