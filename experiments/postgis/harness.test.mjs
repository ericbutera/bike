import { test } from "node:test";
import assert from "node:assert/strict";
import {
  configuration,
  validate,
  correctness,
  summarize,
  tileBounds,
  discoverySQL,
  imageIdentity,
  resolvedLimits,
} from "./harness.mjs";

test("Compose string memory values and numeric limits retain changes in the comparison controls", () => {
  const services = {
    database: {
      cpus: 2,
      mem_limit: "3221225472",
      command: [
        "postgres",
        "-c",
        "shared_buffers=256MB",
        "-c",
        "work_mem=16MB",
        "-c",
        "jit=off",
        "-c",
        "max_parallel_workers_per_gather=0",
        "-c",
        "autovacuum=off",
      ],
    },
    probe: { cpus: 2, mem_limit: "3221225472" },
  };
  assert.equal(resolvedLimits(services).postgres_memory_bytes, 3221225472);
  assert.equal(resolvedLimits(services).jit, false);
  services.database.cpus = 1;
  services.probe.mem_limit = "2147483648";
  assert.equal(resolvedLimits(services).postgres_cpus, 1);
  assert.equal(resolvedLimits(services).probe_memory_bytes, 2147483648);
});

test("image comparisons ignore only the isolated run label, preserving layers and runtime settings", () => {
  const info = {
    Architecture: "arm64",
    Os: "linux",
    RootFS: { Layers: ["layer-1"] },
    Config: {
      Env: ["PG_MAJOR=17"],
      Labels: {
        "com.docker.compose.project": "run-a",
        "com.docker.compose.service": "database",
      },
    },
  };
  const rerun = structuredClone(info);
  rerun.Config.Labels["com.docker.compose.project"] = "run-b";
  assert.equal(imageIdentity(info), imageIdentity(rerun));
  rerun.RootFS.Layers.push("changed-library");
  assert.notEqual(imageIdentity(info), imageIdentity(rerun));
  const runtime = structuredClone(info);
  runtime.Config.Env = ["PG_MAJOR=18"];
  assert.notEqual(imageIdentity(info), imageIdentity(runtime));
});

test("negative coordinates cannot become an SQL comment in regional selection", () => {
  const sql = discoverySQL([-54, -166]);
  assert.ok(sql.includes("- (-54)"));
  assert.ok(sql.includes("- (-166)"));
  assert.ok(!sql.includes("--"));
  assert.throws(() => discoverySQL([10, Infinity]));
});

test("smoke config retains hypothesis links and reports its smaller sample scope", async () => {
  const config = await configuration(
    new URL("./experiments/EXP001-smoke.json", import.meta.url),
  );
  assert.equal(config.samples_per_batch, 1);
  assert.equal(config.batches, 1);
  assert.deepEqual(config.corpora, ["history-1x"]);
  assert.equal(config.ideas.length, 3);
  assert.ok(config.limitations.some((l) => l.includes("One observation")));
  assert.throws(() => validate({ ...config, arms: ["P1"] }));
  assert.throws(() => validate({ ...config, endpoint_radius_meters: 50 }));
});
test("a missing baseline or output mismatch invalidates correctness", () => {
  const summaries = [
    {
      corpus: "history-1x",
      case: "race",
      arm: "V0",
      fingerprint: "a",
      within_arm_correct: true,
    },
    {
      corpus: "history-1x",
      case: "race",
      arm: "P1",
      fingerprint: "b",
      within_arm_correct: true,
    },
  ];
  assert.deepEqual(
    correctness(summaries).map((c) => c.passed),
    [true, false],
  );
  assert.equal(correctness(summaries.slice(1))[0].passed, false);
});
test("warm observations aggregate across batches; pilot samples do not get a p95", () => {
  const batch = {
    corpus: "history-1x",
    case: "detail",
    kind: "detail",
    arm: "V0",
    pg_batch_cpu_ms: 6,
    rust_process_peak_rss_bytes: 100,
    rust_rss_after_bytes: 90,
    pg_after: { memory_current_bytes: 1000 },
  };
  const observation = {
    fingerprint: "same",
    sql_transfer_ms: 1,
    decode_ms: 2,
    domain_ms: 0,
    output_ms: 1,
    rust_cpu_ms: 3,
    payload_bytes: 40,
    candidate_rows: 1,
    vertices: 5,
    sql_calls: 2,
    output_bytes: 10,
  };
  const summary = summarize([
    { ...batch, observations: [{ ...observation, elapsed_ms: 8 }] },
    { ...batch, observations: [{ ...observation, elapsed_ms: 4 }] },
  ])[0];
  assert.equal(summary.samples, 2);
  assert.equal(summary.median_ms, 4);
  assert.equal(summary.p95_ms, null);
  assert.equal(summary.pg_cpu_ms, 6);
  assert.equal(summary.within_arm_correct, true);
});
test("tile gutter expands the actual viewport", () => {
  const bare = tileBounds([40, -80], 16, 256, 0);
  const padded = tileBounds([40, -80], 16, 256, 8);
  assert.ok(
    padded[0] < bare[0] &&
      padded[1] < bare[1] &&
      padded[2] > bare[2] &&
      padded[3] > bare[3],
  );
});
