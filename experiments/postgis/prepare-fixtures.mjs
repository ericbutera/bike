import { createHash } from "node:crypto";
import { once } from "node:events";
import { createReadStream, createWriteStream } from "node:fs";
import { mkdir, readFile, rename, stat, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { spawn, execFileSync } from "node:child_process";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";
import { createGunzip, createGzip } from "node:zlib";

const root = dirname(fileURLToPath(import.meta.url));
const fixtures = join(root, "fixtures");
const source = join(fixtures, "source.ndjson.gz");
const schema = "postgis_eval";
const tableFor = {
  activity: "activities",
  segment: "segments",
  effort: "segment_efforts",
};

async function fingerprint(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return { sha256: hash.digest("hex"), bytes: (await stat(path)).size };
}

async function snapshot() {
  try {
    await stat(source);
    return;
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  const child = spawn(
    "kubectl",
    [
      "-n",
      "pg",
      "exec",
      "-i",
      "pg-0",
      "--",
      "psql",
      "-X",
      "-qAt",
      "-U",
      "postgres",
      "-d",
      "bike",
      "-v",
      "ON_ERROR_STOP=1",
      "-f",
      "-",
    ],
    { stdio: ["pipe", "pipe", "inherit"] },
  );
  const exited = once(child, "close");
  child.stdin.end(await readFile(join(root, "export.sql")));
  await pipeline(
    child.stdout,
    createGzip(),
    createWriteStream(`${source}.tmp`),
  );
  const [code] = await exited;
  if (code !== 0) throw new Error(`Read-only source export failed: ${code}`);
  await rename(`${source}.tmp`, source);
}

async function* records() {
  const input = createReadStream(source).pipe(createGunzip());
  let pending = [];
  let depth = 0;
  let quoted = false;
  let escaped = false;
  for await (const line of createInterface({ input, crlfDelay: Infinity })) {
    if (!line && pending.length === 0) continue;
    if (pending.length === 0) {
      try {
        yield JSON.parse(line);
        continue;
      } catch {
        // PostgreSQL json_agg can pretty-print composite metadata over lines.
      }
    }
    pending.push(line);
    for (const character of line) {
      if (escaped) escaped = false;
      else if (quoted && character === "\\") escaped = true;
      else if (character === '"') quoted = !quoted;
      else if (!quoted && (character === "{" || character === "[")) depth++;
      else if (!quoted && (character === "}" || character === "]")) depth--;
    }
    if (depth === 0 && !quoted) {
      yield JSON.parse(pending.join("\n"));
      pending = [];
    }
  }
  if (pending.length) throw new Error("Incomplete source JSON record");
}

function points(row, kind) {
  const value =
    kind === "activity" ? row.derived_data_json : row.route_data_json;
  if (kind === "activity") {
    if (value == null) return [];
    if (value.v !== 2)
      throw new Error("Fixture generator expects Rust v2 routes");
    return value.route_points ?? [];
  }
  return value ?? [];
}

function valid(point) {
  return (
    Number.isFinite(point.latitude) &&
    Number.isFinite(point.longitude) &&
    Math.abs(point.latitude) <= 90 &&
    Math.abs(point.longitude) <= 180
  );
}

function distanceKm(a, b) {
  const rad = Math.PI / 180;
  const x =
    Math.sin(((b[0] - a[0]) * rad) / 2) ** 2 +
    Math.cos(a[0] * rad) *
      Math.cos(b[0] * rad) *
      Math.sin(((b[1] - a[1]) * rad) / 2) ** 2;
  return 12742 * Math.asin(Math.min(1, Math.sqrt(x)));
}

function quantiles(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const at = (p) => sorted[Math.ceil(p * sorted.length) - 1] ?? 0;
  return {
    min: sorted[0] ?? 0,
    p50: at(0.5),
    p95: at(0.95),
    p99: at(0.99),
    max: at(1),
  };
}

function increment(map, key, amount = 1) {
  map[key] = (map[key] ?? 0) + amount;
}

function shiftYears(value, years) {
  if (value == null || years === 0) return value;
  const date = new Date(value);
  const month = date.getUTCMonth();
  date.setUTCFullYear(date.getUTCFullYear() + years);
  if (date.getUTCMonth() !== month) date.setUTCDate(0); // Feb 29 -> Feb 28.
  return date.toISOString();
}

async function inspect() {
  let metadata;
  const ids = { activity: new Map(), segment: new Map(), effort: new Map() };
  const profile = {
    by_year: {},
    by_month: {},
    by_sport: {},
    route_points: [],
    no_route: 0,
    total_route_points: 0,
    valid_route_points: 0,
    chart_points: [],
    regions: [],
    activities: [],
    efforts_by_segment: {},
    invalid_effort_windows: 0,
    orphan_activity_refs: 0,
    orphan_activity_ids: new Map(),
    users: new Set(),
    segment_point_counts: [],
  };
  for await (const { kind, data } of records()) {
    if (kind === "metadata") {
      metadata = data;
      continue;
    }
    ids[kind].set(data.id, ids[kind].size + 1);
    profile.users.add(data.user_id);
    if (kind === "effort") {
      increment(profile.efforts_by_segment, ids.segment.get(data.segment_id));
      const activity =
        profile.activities[ids.activity.get(data.activity_id) - 1];
      if (!activity) {
        profile.orphan_activity_refs++;
        if (!profile.orphan_activity_ids.has(data.activity_id)) {
          profile.orphan_activity_ids.set(
            data.activity_id,
            profile.orphan_activity_ids.size + 1,
          );
        }
        continue;
      }
      if (!ids.segment.has(data.segment_id)) throw new Error("Missing segment");
      if (
        data.start_route_point_index < 0 ||
        data.end_route_point_index >= activity.points ||
        data.start_route_point_index > data.end_route_point_index
      ) {
        profile.invalid_effort_windows++;
      }
      continue;
    }
    const route = points(data, kind);
    if (kind === "segment") {
      profile.segment_point_counts.push(route.length);
      continue;
    }
    const usable = route.filter(valid);
    profile.total_route_points += route.length;
    profile.valid_route_points += usable.length;
    profile.route_points.push(route.length);
    profile.chart_points.push(
      data.derived_data_json?.chart_points?.length ?? 0,
    );
    if (usable.length < 2) profile.no_route++;
    increment(profile.by_year, data.started_at.slice(0, 4));
    increment(profile.by_month, data.started_at.slice(5, 7));
    increment(profile.by_sport, data.sport);
    let region = null;
    let bounds = null;
    if (usable.length >= 2) {
      const start = [usable[0].latitude, usable[0].longitude];
      let found = profile.regions.find(
        (candidate) => distanceKm(candidate.center, start) <= 80,
      );
      if (!found) {
        found = {
          id: `R${profile.regions.length + 1}`,
          center: start,
          activities: 0,
        };
        profile.regions.push(found);
      }
      found.activities++;
      region = found.id;
      bounds = [Infinity, Infinity, -Infinity, -Infinity];
      for (const point of usable) {
        bounds[0] = Math.min(bounds[0], point.longitude);
        bounds[1] = Math.min(bounds[1], point.latitude);
        bounds[2] = Math.max(bounds[2], point.longitude);
        bounds[3] = Math.max(bounds[3], point.latitude);
      }
    }
    profile.activities.push({
      id: ids.activity.get(data.id),
      started_at: data.started_at,
      points: route.length,
      region,
      bounds,
      sport: data.sport,
    });
  }
  if (
    !metadata ||
    ids.activity.size !== metadata.activity_count ||
    ids.segment.size !== metadata.segment_count ||
    ids.effort.size !== metadata.effort_count
  ) {
    throw new Error("Snapshot row counts differ from metadata");
  }
  profile.activities.sort(
    (a, b) => Date.parse(a.started_at) - Date.parse(b.started_at),
  );
  const first = profile.activities[0].started_at;
  const last = profile.activities.at(-1).started_at;
  profile.block_years = Math.ceil(
    (Date.parse(last) - Date.parse(first) + 86400000) / (365.2425 * 86400000),
  );
  profile.home = [...profile.regions].sort(
    (a, b) => b.activities - a.activities,
  )[0]?.id;
  profile.travel_episodes = 0;
  let previous = null;
  for (const activity of profile.activities) {
    if (!activity.region) continue;
    if (activity.region !== profile.home && activity.region !== previous)
      profile.travel_episodes++;
    previous = activity.region;
  }
  return { metadata, ids, profile, first, last };
}

function copyValue(value, sqlType) {
  if (value == null) return "\\N";
  const text =
    sqlType === "json" || sqlType === "jsonb"
      ? JSON.stringify(value)
      : typeof value === "boolean"
        ? value
          ? "t"
          : "f"
        : String(value);
  return text
    .replaceAll("\\", "\\\\")
    .replaceAll("\t", "\\t")
    .replaceAll("\n", "\\n")
    .replaceAll("\r", "\\r");
}

async function dump(corpus, blocks) {
  const { metadata, ids, profile } = corpus;
  const name = `history-${blocks}x`;
  const path = join(fixtures, `${name}.sql.gz`);
  const gzip = createGzip();
  const target = createWriteStream(path);
  const completion = pipeline(gzip, target);
  async function put(text) {
    if (!gzip.write(text)) await once(gzip, "drain");
  }
  const columns = Object.fromEntries(
    Object.values(tableFor).map((table) => [
      table,
      metadata.columns.filter((column) => column.table_name === table),
    ]),
  );
  await put(
    `-- Synthetic identities; actual route/telemetry distribution. Disposable database only.\nBEGIN;\nCREATE SCHEMA ${schema};\nCREATE TABLE ${schema}.users (id integer PRIMARY KEY, name text NOT NULL);\n`,
  );
  const users = new Map(
    [...profile.users]
      .sort((a, b) => a - b)
      .map((id, index) => [id, index + 1]),
  );
  for (const id of users.values())
    await put(
      `INSERT INTO ${schema}.users VALUES (${id}, 'Fixture rider ${id}');\n`,
    );
  for (const [table, list] of Object.entries(columns)) {
    for (const column of list) {
      if (!/^[a-z_]+$/.test(column.column_name))
        throw new Error("Unexpected column name");
    }
    await put(
      `CREATE TABLE ${schema}.${table} (\n${list
        .map(
          (column) =>
            `  ${column.column_name} ${column.sql_type}${column.column_name === "id" ? " PRIMARY KEY" : column.is_nullable === "NO" ? " NOT NULL" : ""}`,
        )
        .join(",\n")}\n);\n`,
    );
  }
  for (let block = 0; block < blocks; block++) {
    let active = null;
    for await (const { kind, data } of records()) {
      if (kind === "metadata" || (kind === "segment" && block > 0)) continue;
      const table = tableFor[kind];
      if (active !== table) {
        if (active) await put("\\.\n");
        active = table;
        await put(
          `COPY ${schema}.${table} (${columns[table].map((column) => column.column_name).join(", ")}) FROM stdin;\n`,
        );
      }
      data.id =
        ids[kind].get(data.id) +
        (kind === "segment" ? 0 : block * ids[kind].size);
      data.user_id = users.get(data.user_id);
      const durationMs =
        kind === "activity" && data.ended_at
          ? Date.parse(data.ended_at) - Date.parse(data.started_at)
          : null;
      for (const field of [
        "started_at",
        "ended_at",
        "created_at",
        "updated_at",
      ]) {
        if (data[field])
          data[field] = shiftYears(data[field], block * profile.block_years);
      }
      if (kind === "activity") {
        if (durationMs !== null)
          data.ended_at = new Date(
            Date.parse(data.started_at) + durationMs,
          ).toISOString();
        data.title = `Fixture activity ${data.id}`;
        data.source = "fixture";
        data.source_correlation_id = null;
        data.original_filename = null;
        data.activity_import_id = null;
        for (const lap of data.derived_data_json?.laps ?? [])
          lap.title = `Fixture lap ${lap.lap_index}`;
      } else if (kind === "segment") {
        data.title = `Fixture segment ${data.id}`;
        data.source = "fixture";
        data.original_filename = null;
        data.source_activity_id =
          ids.activity.get(data.source_activity_id) ?? null;
      } else {
        const mappedActivity = ids.activity.get(data.activity_id);
        data.activity_id =
          mappedActivity === undefined
            ? -profile.orphan_activity_ids.get(data.activity_id) -
              block * profile.orphan_activity_ids.size
            : mappedActivity + block * ids.activity.size;
        data.segment_id = ids.segment.get(data.segment_id);
        if (blocks > 1) {
          data.overall_rank = null;
          data.user_rank = null;
        }
      }
      await put(
        columns[table]
          .map((column) => copyValue(data[column.column_name], column.sql_type))
          .join("\t") + "\n",
      );
    }
    if (active) await put("\\.\n");
  }
  for (const index of metadata.indexes) {
    await put(index.replace(/ON public\./, `ON ${schema}.`) + ";\n");
  }
  await put(
    `COMMIT;\nANALYZE ${schema}.activities;\nANALYZE ${schema}.segments;\nANALYZE ${schema}.segment_efforts;\n`,
  );
  gzip.end();
  await completion;
  const manifest = {
    name,
    purpose:
      blocks === 1
        ? "Observed history with synthetic identities"
        : "3x history-growth sensitivity; not a forecast",
    source_snapshot_at: metadata.snapshot_at,
    source_postgres_version: metadata.server_version,
    source_live_relation_bytes: metadata.live_relation_bytes,
    source_revision: execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: root,
      encoding: "utf8",
    }).trim(),
    source_fingerprint: await fingerprint(source),
    dump: await fingerprint(path),
    schema,
    activity_count: ids.activity.size * blocks,
    segment_count: ids.segment.size,
    effort_count: ids.effort.size * blocks,
    owner_count: users.size,
    first_activity: corpus.first,
    last_activity: shiftYears(corpus.last, (blocks - 1) * profile.block_years),
    calendar_block_years: profile.block_years,
    blocks,
    source_by_year: profile.by_year,
    source_by_month: profile.by_month,
    source_by_sport: profile.by_sport,
    total_route_points: profile.total_route_points * blocks,
    valid_route_points: profile.valid_route_points * blocks,
    route_point_quantiles: quantiles(profile.route_points),
    chart_point_quantiles: quantiles(profile.chart_points),
    segment_point_quantiles: quantiles(profile.segment_point_counts),
    activities_without_two_valid_points: profile.no_route * blocks,
    source_region_counts: profile.regions.map(({ id, activities }) => ({
      id,
      activities,
    })),
    source_home_region: profile.home,
    source_non_home_region_episodes: profile.travel_episodes,
    region_method:
      "First valid GPS point; first-fit 80km radius; descriptive proxy, not geocoded trips",
    source_invalid_effort_windows: profile.invalid_effort_windows,
    source_orphan_activity_efforts: profile.orphan_activity_refs,
    expected_orphan_activity_efforts: profile.orphan_activity_refs * blocks,
    synthetic_changes: [
      "IDs/names/titles/source identifiers replaced; import links cleared",
      "Routes, telemetry, sports, effort windows/durations retained",
      `Growth adds calendar-year-shifted copies; same ${ids.segment.size} physical segments`,
      "Source orphan references retained as synthetic negative IDs",
      "Growth stored ranks cleared; baseline ranks retained",
      "JSON formatting normalized; storage is not a byte-for-byte production copy",
    ],
  };
  await writeFile(
    join(fixtures, `${name}.manifest.json`),
    JSON.stringify(manifest, null, 2) + "\n",
  );
  console.log(
    `${name}: ${manifest.activity_count} activities, ${manifest.effort_count} efforts, ${(manifest.dump.bytes / 1048576).toFixed(1)} MiB compressed`,
  );
}

async function verify() {
  for (const blocks of [1, 3]) {
    const name = `history-${blocks}x`;
    const manifest = JSON.parse(
      await readFile(join(fixtures, `${name}.manifest.json`), "utf8"),
    );
    const actual = await fingerprint(join(fixtures, `${name}.sql.gz`));
    if (
      actual.sha256 !== manifest.dump.sha256 ||
      actual.bytes !== manifest.dump.bytes
    )
      throw new Error(`Dump fingerprint mismatch: ${name}`);
    const rows = { activities: 0, segments: 0, segment_efforts: 0 };
    let table = null;
    const input = createReadStream(join(fixtures, `${name}.sql.gz`)).pipe(
      createGunzip(),
    );
    for await (const line of createInterface({ input, crlfDelay: Infinity })) {
      if (line.startsWith(`COPY ${schema}.`))
        table = line.split(" ")[1].split(".")[1];
      else if (line === "\\.") table = null;
      else if (table) rows[table]++;
    }
    if (
      rows.activities !== manifest.activity_count ||
      rows.segments !== manifest.segment_count ||
      rows.segment_efforts !== manifest.effort_count
    )
      throw new Error(`COPY row count mismatch: ${name}`);
    console.log(`${name}: checksum, gzip stream, and COPY counts verified`);
  }
}

await mkdir(fixtures, { recursive: true });
if (process.argv.includes("--verify")) {
  await verify();
} else {
  await snapshot();
  const corpus = await inspect();
  const byPoints = [...corpus.profile.activities]
    .filter((activity) => activity.bounds)
    .sort((a, b) => a.points - b.points);
  const cases = {
    details: [
      byPoints[Math.floor(byPoints.length / 2)],
      byPoints[Math.floor(byPoints.length * 0.95)],
      byPoints.at(-1),
    ],
    regions: corpus.profile.regions,
    segments_by_effort_count: Object.entries(
      corpus.profile.efforts_by_segment,
    ).sort((a, b) => b[1] - a[1]),
    note: "Private coordinates/IDs for workload selection; geometry remains in SQL fixtures",
  };
  await writeFile(
    join(fixtures, "private-cases.json"),
    JSON.stringify(cases, null, 2) + "\n",
  );
  await dump(corpus, 1);
  await dump(corpus, 3);
  await verify();
}
