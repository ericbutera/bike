import { spawn, execFileSync } from "node:child_process";
import { once } from "node:events";
import { createReadStream } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";
import { createGunzip } from "node:zlib";

const fixtures = join(dirname(fileURLToPath(import.meta.url)), "fixtures");
const container = "bike-postgis-fixture-check";
const info = JSON.parse(
  execFileSync("docker", ["inspect", container], { encoding: "utf8" }),
)[0];
if (info.HostConfig.NetworkMode !== "none")
  throw new Error("Restore checks require the isolated fixture container");
execFileSync("docker", ["exec", container, "pg_isready", "-U", "postgres"]);

function psql(database, sql) {
  return execFileSync(
    "docker",
    [
      "exec",
      "-i",
      container,
      "psql",
      "-X",
      "-qAt",
      "-U",
      "postgres",
      "-d",
      database,
      "-v",
      "ON_ERROR_STOP=1",
    ],
    {
      input: sql,
      encoding: "utf8",
      maxBuffer: 1024 * 1024,
    },
  ).trim();
}

const verification = {
  phase: "fixture_restore_verification_only",
  postgres_version: psql("postgres", "SELECT version();"),
  image_id: info.Image,
  limits: {
    cpu_nanocpus: info.HostConfig.NanoCpus,
    memory_bytes: info.HostConfig.Memory,
  },
  postgis_benchmark_run: false,
  corpora: [],
};

for (const blocks of [1, 3]) {
  const name = `history-${blocks}x`;
  const manifest = JSON.parse(
    await readFile(join(fixtures, `${name}.manifest.json`), "utf8"),
  );
  const database = `postgis_eval_restore_${blocks}x`;
  if (
    psql(
      "postgres",
      `SELECT count(*) FROM pg_database WHERE datname='${database}';`,
    ) !== "0"
  )
    throw new Error(
      "Use a fresh disposable container to repeat restore checks",
    );
  execFileSync("docker", [
    "exec",
    container,
    "createdb",
    "-U",
    "postgres",
    database,
  ]);
  const child = spawn(
    "docker",
    [
      "exec",
      "-i",
      container,
      "psql",
      "-X",
      "-q",
      "-U",
      "postgres",
      "-d",
      database,
      "-v",
      "ON_ERROR_STOP=1",
    ],
    { stdio: ["pipe", "ignore", "inherit"] },
  );
  const exited = once(child, "close");
  await pipeline(
    createReadStream(join(fixtures, `${name}.sql.gz`)),
    createGunzip(),
    child.stdin,
  );
  const [code] = await exited;
  if (code !== 0) throw new Error(`Restore failed: ${name}`);
  const result = JSON.parse(
    psql(
      database,
      `
    SELECT jsonb_build_object(
      'activities', (SELECT count(*) FROM postgis_eval.activities),
      'segments', (SELECT count(*) FROM postgis_eval.segments),
      'efforts', (SELECT count(*) FROM postgis_eval.segment_efforts),
      'route_points', (SELECT sum(coalesce(json_array_length(derived_data_json->'route_points'), 0)) FROM postgis_eval.activities),
      'orphan_activities', (SELECT count(*) FROM postgis_eval.segment_efforts e LEFT JOIN postgis_eval.activities a ON a.id=e.activity_id WHERE a.id IS NULL),
      'orphan_segments', (SELECT count(*) FROM postgis_eval.segment_efforts e LEFT JOIN postgis_eval.segments s ON s.id=e.segment_id WHERE s.id IS NULL),
      'invalid_windows', (SELECT count(*) FROM postgis_eval.segment_efforts e JOIN postgis_eval.activities a ON a.id=e.activity_id WHERE e.start_route_point_index < 0 OR e.end_route_point_index >= coalesce(json_array_length(a.derived_data_json->'route_points'), 0) OR e.end_route_point_index < e.start_route_point_index),
      'owner_mismatches', (SELECT count(*) FROM postgis_eval.segment_efforts e JOIN postgis_eval.activities a ON a.id=e.activity_id WHERE a.user_id <> e.user_id),
      'non_synthetic_rows', (SELECT count(*) FROM postgis_eval.activities WHERE source <> 'fixture' OR title NOT LIKE 'Fixture activity %' OR activity_import_id IS NOT NULL OR original_filename IS NOT NULL OR source_correlation_id IS NOT NULL),
      'growth_payload_mismatches', (SELECT count(*) FROM (SELECT ((id-1) % ${manifest.activity_count / blocks})+1 AS original_id FROM postgis_eval.activities GROUP BY original_id HAVING count(DISTINCT md5(derived_data_json::text)) <> 1) x),
      'growth_duration_mismatches', (SELECT count(*) FROM postgis_eval.activities a JOIN postgis_eval.activities original ON original.id=((a.id-1) % ${manifest.activity_count / blocks})+1 WHERE (a.ended_at-a.started_at) IS DISTINCT FROM (original.ended_at-original.started_at)),
      'database_bytes', pg_database_size(current_database()),
      'relations', (SELECT jsonb_agg(jsonb_build_object('table', c.relname, 'heap_bytes', pg_relation_size(c.oid), 'table_including_toast_bytes', pg_table_size(c.oid), 'ordinary_indexes_bytes', pg_indexes_size(c.oid), 'total_bytes', pg_total_relation_size(c.oid)) ORDER BY c.relname) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='postgis_eval' AND c.relkind='r')
    );
  `,
    ),
  );
  const expected = {
    activities: manifest.activity_count,
    segments: manifest.segment_count,
    efforts: manifest.effort_count,
    route_points: manifest.total_route_points,
    orphan_activities: manifest.expected_orphan_activity_efforts,
    orphan_segments: 0,
    invalid_windows: manifest.source_invalid_effort_windows * blocks,
    owner_mismatches: 0,
    non_synthetic_rows: 0,
    growth_payload_mismatches: 0,
    growth_duration_mismatches: 0,
  };
  for (const [key, value] of Object.entries(expected))
    if (result[key] !== value)
      throw new Error(`${name}: ${key} expected ${value}, got ${result[key]}`);
  verification.corpora.push({
    name,
    dump_sha256: manifest.dump.sha256,
    status: "passed",
    ...result,
  });
  console.log(
    `${name}: SQL restore, counts, relationships/source anomalies, and growth payload/duration checks passed`,
  );
}
await writeFile(
  join(fixtures, "verification.json"),
  JSON.stringify(verification, null, 2) + "\n",
);
