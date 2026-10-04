import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export async function configuration(file) {
  if (file instanceof URL) file = fileURLToPath(file);
  const own = JSON.parse(await readFile(file, "utf8"));
  const parent = own.extends
    ? await configuration(resolve(dirname(file), own.extends))
    : {};
  const config = {
    ...parent,
    ...own,
    limitations: [...(parent.limitations ?? []), ...(own.limitations ?? [])],
  };
  delete config.extends;
  validate(config);
  return config;
}

export function validate(config) {
  if (
    !/^[A-Z0-9-]+$/i.test(config.id) ||
    !Number.isInteger(config.revision) ||
    config.revision < 1
  )
    throw new Error("Experiment requires an ID and positive revision");
  if (
    !config.corpora?.length ||
    config.corpora.some((name) => !["history-1x", "history-3x"].includes(name))
  )
    throw new Error("Unknown fixture corpus");
  if (
    !config.arms?.includes("V0") ||
    config.arms.some((arm) => !["V0", "V1", "P0", "P1"].includes(arm)) ||
    new Set(config.arms).size !== config.arms.length
  )
    throw new Error("Arms require a unique V0 baseline");
  for (const field of [
    "samples_per_batch",
    "batches",
    "tile_size",
    "tile_gutter",
    "tile_zoom",
  ])
    if (!Number.isInteger(config[field]) || config[field] < 1)
      throw new Error(`Invalid ${field}`);
  if (
    !Number.isInteger(config.warmups) ||
    config.warmups < 0 ||
    !Number.isFinite(config.endpoint_radius_meters) ||
    config.endpoint_radius_meters < 263
  )
    throw new Error("Invalid warmup count or unsafe endpoint radius");
  if (
    config.tile_size > 1024 ||
    config.tile_gutter > 64 ||
    config.tile_zoom > 20
  )
    throw new Error("Unbounded tile parameters");
}

export function tileBounds(center, zoom, size, gutter) {
  const [lat, lon] = center;
  const scale = 2 ** zoom;
  const x = Math.floor(((lon + 180) / 360) * scale);
  const y = Math.floor(
    ((1 - Math.asinh(Math.tan((lat * Math.PI) / 180)) / Math.PI) / 2) * scale,
  );
  const longitude = (v) => (v / scale) * 360 - 180;
  const latitude = (v) =>
    (Math.atan(Math.sinh(Math.PI * (1 - (2 * v) / scale))) * 180) / Math.PI;
  const pad = gutter / size;
  return [
    longitude(x - pad),
    latitude(y + 1 + pad),
    longitude(x + 1 + pad),
    latitude(y - pad),
  ];
}

export function discoverySQL(center) {
  const [lat, lon] = center;
  if (
    !Number.isFinite(lat) ||
    !Number.isFinite(lon) ||
    Math.abs(lat) > 90 ||
    Math.abs(lon) > 180
  )
    throw new Error("Invalid regional selector");
  return `SELECT jsonb_build_object('route_free',(SELECT min(id) FROM postgis_eval.activities WHERE coalesce(json_array_length(derived_data_json->'route_points'),0)<2),'busy',(SELECT segment_id FROM postgis_eval.segment_efforts GROUP BY segment_id ORDER BY count(*) DESC,segment_id LIMIT 1),'sparse',(SELECT segment_id FROM postgis_eval.segment_efforts GROUP BY segment_id ORDER BY count(*),segment_id LIMIT 1),'other_region',(SELECT id FROM postgis_eval.activities WHERE json_array_length(derived_data_json->'route_points')>=2 AND abs((derived_data_json->'route_points'->0->>'latitude')::double precision - (${lat}))<0.5 AND abs((derived_data_json->'route_points'->0->>'longitude')::double precision - (${lon}))<0.5 ORDER BY id LIMIT 1))`;
}

export function workloadCases(config, selector, manifest, discovery) {
  const regions = [...selector.regions].sort(
    (a, b) => b.activities - a.activities,
  );
  const latest = new Date(manifest.last_activity);
  const recent = new Date(latest.getTime() - 90 * 86400000).toISOString();
  const base = {
    warmups: config.warmups,
    samples: config.samples_per_batch,
    radius: config.endpoint_radius_meters,
    size: config.tile_size,
    gutter: config.tile_gutter,
    from: "0001-01-01T00:00:00Z",
    bbox: null,
    tile_bbox: null,
    id: 0,
  };
  const cases = [];
  const add = (name, kind, values = {}) =>
    cases.push({ ...base, name, kind, ...values });
  const labels = ["median", "p95", "maximum"];
  selector.details.forEach((detail, index) => {
    add(`detail-${labels[index]}`, "detail", { id: detail.id });
    add(`map-input-${labels[index]}`, "map_inputs", { id: detail.id });
  });
  add("detail-route-free", "detail", { id: discovery.route_free });
  add("race-busy", "race", { id: discovery.busy });
  add("race-sparse", "race", { id: discovery.sparse });
  add("match-local", "segment_match", { id: selector.details[0].id });
  add("match-long", "segment_match", { id: selector.details[2].id });
  add("match-other-region", "segment_match", { id: discovery.other_region });
  add("segment-history-busy", "segment_history", { id: discovery.busy });
  for (const [name, center] of [
    ["home", regions[0].center],
    ["other-region", regions[1].center],
    ["empty", [0, -150]],
  ]) {
    add(`heatmap-${name}`, "heatmap", {
      bbox: tileBounds(
        center,
        config.tile_zoom,
        config.tile_size,
        config.tile_gutter,
      ),
      tile_bbox: tileBounds(center, config.tile_zoom, config.tile_size, 0),
    });
  }
  add("heatmap-home-90days", "heatmap", {
    bbox: cases.find((c) => c.name === "heatmap-home").bbox,
    tile_bbox: cases.find((c) => c.name === "heatmap-home").tile_bbox,
    from: recent,
  });
  if (cases.some((c) => c.kind !== "heatmap" && !Number.isInteger(c.id)))
    throw new Error("Fixture lacks a selected workload");
  return cases;
}

export function percentile(values, fraction) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil(sorted.length * fraction) - 1)];
}

export function summarize(batches) {
  const groups = new Map();
  for (const batch of batches) {
    const key = [batch.corpus, batch.case, batch.arm].join("/");
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(batch);
  }
  return [...groups.entries()]
    .map(([key, runs]) => {
      const observations = runs.flatMap((r) => r.observations);
      const elapsed = observations.map((o) => o.elapsed_ms);
      const first = observations[0];
      const med = (field) =>
        percentile(
          observations.map((o) => o[field]),
          0.5,
        );
      return {
        key,
        corpus: runs[0].corpus,
        case: runs[0].case,
        kind: runs[0].kind,
        arm: runs[0].arm,
        samples: elapsed.length,
        median_ms: percentile(elapsed, 0.5),
        p95_ms: elapsed.length >= 100 ? percentile(elapsed, 0.95) : null,
        min_ms: Math.min(...elapsed),
        max_ms: Math.max(...elapsed),
        sql_transfer_ms: med("sql_transfer_ms"),
        decode_ms: med("decode_ms"),
        domain_ms: med("domain_ms"),
        output_ms: med("output_ms"),
        rust_cpu_ms: med("rust_cpu_ms"),
        pg_cpu_ms:
          runs.reduce((sum, r) => sum + r.pg_batch_cpu_ms, 0) / elapsed.length,
        rust_process_peak_rss_bytes: Math.max(
          ...runs.map((r) => r.rust_process_peak_rss_bytes),
        ),
        rust_retained_rss_bytes: Math.max(
          ...runs.map((r) => r.rust_rss_after_bytes),
        ),
        pg_memory_current_bytes: Math.max(
          ...runs.map((r) => r.pg_after.memory_current_bytes),
        ),
        payload_bytes: med("payload_bytes"),
        candidate_rows: med("candidate_rows"),
        vertices: med("vertices"),
        sql_calls: med("sql_calls"),
        output_bytes: med("output_bytes"),
        fingerprint: first.fingerprint,
        within_arm_correct: observations.every(
          (o) => o.fingerprint === first.fingerprint,
        ),
      };
    })
    .sort((a, b) => a.key.localeCompare(b.key));
}

export function correctness(summaries) {
  const baseline = new Map(
    summaries
      .filter((s) => s.arm === "V0")
      .map((s) => [[s.corpus, s.case].join("/"), s.fingerprint]),
  );
  return summaries.map((s) => ({
    corpus: s.corpus,
    case: s.case,
    arm: s.arm,
    passed:
      s.within_arm_correct &&
      baseline.get([s.corpus, s.case].join("/")) === s.fingerprint,
    fingerprint: s.fingerprint,
  }));
}

export function compatibility(run) {
  return {
    database_image:
      run.environment.database_image.content_sha256 ??
      run.environment.database_image.id,
    architecture: run.environment.docker.Architecture,
    cpu_count: run.environment.docker.NCPU,
    memory: run.environment.docker.MemTotal,
    limits: run.environment.limits,
    config: {
      ...run.config,
      id: undefined,
      title: undefined,
      change: undefined,
      revision: undefined,
      limitations: undefined,
    },
    fixtures: run.fixtures,
    workloads: run.workloads,
    probe_toolchain: run.environment.probe_toolchain,
    postgres_settings: run.environment.postgres_settings,
    compose: run.environment.compose,
    context: run.environment.context,
    kernel: run.environment.docker.KernelVersion,
    driver: run.environment.docker.Driver,
    server: run.environment.docker.ServerVersion,
  };
}

export function imageIdentity(info) {
  const config = { ...info.Config, Labels: { ...info.Config.Labels } };
  // Compose injects this unique run label into otherwise identical images.
  // Keep all other runtime configuration and every filesystem layer controlled.
  delete config.Labels["com.docker.compose.project"];
  return digest({
    architecture: info.Architecture,
    os: info.Os,
    layers: info.RootFS.Layers,
    config,
  });
}

export function resolvedLimits(services) {
  const settings = Object.fromEntries(
    services.database.command
      .filter((arg) => arg.includes("="))
      .map((arg) => arg.split("=")),
  );
  return {
    postgres_cpus: Number(services.database.cpus),
    postgres_memory_bytes: Number(services.database.mem_limit),
    probe_cpus: Number(services.probe.cpus),
    probe_memory_bytes: Number(services.probe.mem_limit),
    shared_buffers: settings.shared_buffers,
    work_mem: settings.work_mem,
    jit: settings.jit === "on",
    parallel_workers_per_gather: Number(
      settings.max_parallel_workers_per_gather,
    ),
    autovacuum: settings.autovacuum === "on",
  };
}

export function digest(value) {
  const canonical = (item) =>
    Array.isArray(item)
      ? item.map(canonical)
      : item && typeof item === "object"
        ? Object.fromEntries(
            Object.keys(item)
              .sort()
              .map((key) => [key, canonical(item[key])]),
          )
        : item;
  return createHash("sha256")
    .update(JSON.stringify(canonical(value)))
    .digest("hex");
}

export function reportMarkdown(run) {
  const mib = (n) => (n / 1048576).toFixed(1);
  const number = (n) => n?.toFixed(2) ?? "—";
  const lines = [
    `# ${run.config.id} revision ${run.config.revision}: ${run.config.title}`,
    "",
    `Run: \`${run.id}\`; outcome: **${run.outcome}**.`,
    "",
    `Ideas: ${run.config.ideas.join(", ")}. Change under test: ${run.config.change}`,
    "",
    `Source: \`${run.environment.revision}\`; code fingerprint: \`${run.environment.source_sha256}\`.`,
    "",
    `Images: PostgreSQL \`${run.environment.database_image.id}\`; Rust probe \`${run.environment.probe_image.id}\`.`,
    "",
    `Rust toolchain: ${run.environment.probe_toolchain}.`,
    "",
    `Cache: ${run.config.cache}. Warmups: ${run.config.warmups}; observations per case/arm: ${run.config.samples_per_batch * run.config.batches}.`,
    "",
    "## Scope and measurement limits",
    "",
    ...run.config.limitations.map((l) => `- ${l}`),
    "- SQL/transfer includes client protocol decoding and socket wait; separate EXPLAIN plans describe server execution/serialization outside ranked observations.",
    "- Payload bytes count PostgreSQL binary datum bytes, excluding protocol framing and network headers.",
    "- Rust CPU uses getrusage. PG CPU uses isolated server cgroup counter deltas per batch, including counter-query overhead; it is not SQL elapsed time.",
    "- Rust peak RSS is a fresh process lifetime peak per case/batch, including warmup/connection setup. PG memory is a shared server cgroup snapshot, including file cache; it is not per-query peak memory.",
    "- All four arm databases share one otherwise idle server. Its shared buffer/file-cache state and lifetime memory peak must not be interpreted as isolated per-arm memory.",
    "- Private raw observations, parameters and plans remain in this run directory. This pilot does not authorize a product storage decision.",
    "",
    "## Data-access results",
    "",
    "| Corpus | Case | Arm | n | Median ms | p95 ms | Range ms | PG CPU ms/op | Rust CPU ms/op | Rust peak/retained MiB | PG snapshot MiB | Datum KiB | Candidates | Vertices | Output equal |",
    "| --- | --- | --- | ---: | ---: | ---: | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | --- |",
  ];
  for (const s of run.summaries) {
    const valid = run.correctness.find(
      (c) => c.corpus === s.corpus && c.case === s.case && c.arm === s.arm,
    )?.passed;
    lines.push(
      `| ${s.corpus} | ${s.case} | ${s.arm} | ${s.samples} | ${number(s.median_ms)} | ${number(s.p95_ms)} | ${number(s.min_ms)}–${number(s.max_ms)} | ${number(s.pg_cpu_ms)} | ${number(s.rust_cpu_ms)} | ${mib(s.rust_process_peak_rss_bytes)}/${mib(s.rust_retained_rss_bytes)} | ${mib(s.pg_memory_current_bytes)} | ${(s.payload_bytes / 1024).toFixed(1)} | ${s.candidate_rows} | ${s.vertices} | ${valid ? "yes" : "FAILED"} |`,
    );
  }
  lines.push(
    "",
    "## Stage medians",
    "",
    "| Corpus | Case | Arm | SQL + transfer ms | Decode ms | Domain ms | Output ms | SQL calls |",
    "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |",
  );
  for (const s of run.summaries)
    lines.push(
      `| ${s.corpus} | ${s.case} | ${s.arm} | ${number(s.sql_transfer_ms)} | ${number(s.decode_ms)} | ${number(s.domain_ms)} | ${number(s.output_ms)} | ${s.sql_calls} |`,
    );
  lines.push(
    "",
    "## Restored storage and preparation",
    "",
    "| Corpus | Arm | Raw DB MiB | Final DB MiB | Added MiB | Projection/extension preparation ms | PG CPU ms | Rust CPU ms | Rust peak MiB | PostgreSQL/PostGIS |",
    "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |",
  );
  for (const s of run.storage)
    lines.push(
      `| ${s.corpus} | ${s.arm} | ${mib(s.raw_database_bytes)} | ${mib(s.database_bytes)} | ${mib(s.database_bytes - s.raw_database_bytes)} | ${number(s.prepare_elapsed_ms)} | ${number(s.prepare_cpu_ms)} | ${number(s.prepare_rust_cpu_ms)} | ${mib(s.prepare_rust_peak_rss_bytes)} | ${s.postgis ?? "PostGIS disabled"} |`,
    );
  lines.push(
    "",
    "Relation heap/TOAST/index details are in `storage.json`; raw per-operation values in `observations.jsonl`; instrumented server plans in `plans/`. Preparation streams current Rust-decoded coordinates into binary projections in pages of 16, then builds indexes. Preparation CPU and memory are recorded separately; this does not measure the complete activity-import lifecycle.",
    "",
    "## Idea → test → result",
    "",
    "Each idea remains linked to the exact config, fixture hashes, source hashes and output-equality checks in `report.json`. Add reviewed interpretation and a proposed next test to the notebook; timing direction from this pilot alone is not a conclusion.",
  );
  if (run.error) lines.push("", `Failure: ${run.error}`);
  return lines.join("\n") + "\n";
}
