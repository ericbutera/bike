import { segmentSceneData } from "./segment-scene.mjs";

// Playwright owns readable scenario data. The fixture CLI validates every
// record against Bike's current SeaORM model, retaining database defaults.
const created = "2025-01-01T00:00:00Z";
const firstFinish = "2025-06-01T12:02:00Z";
const secondFinish = "2025-06-02T12:01:40Z";
const insert = (entity, values) => ({ entity, operation: "insert", values });
const update = (entity, values) => ({ entity, operation: "update", values });

export const scenarios = [
  "baseline",
  "representative-segment",
  "laps",
  "climb",
  "zones",
  "eleven-efforts",
  "partial-race",
  "archive-states",
  "manual-processing",
  "heatmap",
  "upload-result",
];

function user(id, email, name, options = {}) {
  return insert("users", {
    id,
    pid: `00000000-0000-4000-8000-${String(id).padStart(12, "0")}`,
    email,
    name,
    api_key: `bike-test-user-${id}`,
    is_admin: false,
    disabled: false,
    email_verified_at: created,
    created_at: created,
    updated_at: created,
    ...options,
  });
}

function points(step, elevationStep = 2) {
  return Array.from({ length: 11 }, (_, index) => ({
    elapsed_seconds: index * step,
    latitude: 42 + index * 0.001,
    longitude: -83,
    distance_meters: index * 100,
    elevation_meters: 200 + index * elevationStep,
    speed_mps: Number((100 / step).toFixed(3)),
    heart_rate_bpm: 140 + index,
  }));
}

function ride(id, importId, title, filename, started, step, options = {}) {
  return insert("activities", {
    id,
    user_id: 1,
    activity_import_id: importId,
    title,
    sport: "ride",
    source: "manual_upload",
    original_filename: filename,
    format: "gpx",
    activity_type: "training",
    started_at: started,
    ended_at: new Date(Date.parse(started) + step * 10_000).toISOString(),
    distance_meters: 1000,
    moving_time_seconds: step * 10,
    total_time_seconds: step * 10,
    elevation_gain_meters: 20,
    elevation_loss_meters: 0,
    average_speed_mps: 100 / step,
    max_speed_mps: 12,
    average_heart_rate_bpm: 145,
    max_heart_rate_bpm: 160,
    calories: 100,
    derived_data_json: {
      v: 2,
      route_points: points(step),
      chart_points: points(step),
    },
    created_at: started,
    updated_at: started,
    ...options,
  });
}

function imported(id, activityId, filename, finished) {
  return insert("activity_imports", {
    id,
    user_id: 1,
    source: "manual_upload",
    format: "gpx",
    status: "processed",
    original_filename: filename,
    storage_path: `activity-imports/${filename}`,
    size_bytes: 1268,
    mime_type: "application/gpx+xml",
    activity_id: activityId,
    processing_stage: "complete",
    processing_attempts: 1,
    processed_at: finished,
    created_at: finished,
    updated_at: finished,
  });
}

function artifact(id, filename, checksum, finished) {
  return insert("activity_import_artifacts", {
    id,
    activity_import_id: id,
    user_id: 1,
    artifact_kind: "original",
    format: "gpx",
    source_quality: "gpx_original",
    original_filename: filename,
    storage_path: `activity-imports/${filename}`,
    size_bytes: 1268,
    mime_type: "application/gpx+xml",
    checksum_sha256: checksum,
    created_at: finished,
    updated_at: finished,
  });
}

function effort(id, activityId, duration, rank, index = 1) {
  return insert("segment_efforts", {
    id,
    user_id: 1,
    segment_id: 5,
    activity_id: activityId,
    effort_index: index,
    start_route_point_index: 0,
    end_route_point_index: 10,
    start_elapsed_seconds: 0,
    end_elapsed_seconds: duration,
    duration_seconds: duration,
    distance_meters: 1000,
    overall_rank: rank,
    user_rank: rank,
    created_at: activityId === 1109 ? firstFinish : secondFinish,
    updated_at: secondFinish,
  });
}

export function baselineData() {
  return {
    records: [
      user(1, "developer@bike.local", "Local Developer", { is_admin: true }),
      user(2, "active-rider@bike.local", "Active Parity Rider", {
        created_at: "2025-01-02T00:00:00Z",
        updated_at: "2025-01-02T00:00:00Z",
      }),
      user(3, "disabled-rider@bike.local", "Disabled Parity Rider", {
        disabled: true,
        email_verified_at: null,
        created_at: "2025-01-03T00:00:00Z",
        updated_at: "2025-01-03T00:00:00Z",
      }),
      insert("user_preferences", {
        id: 1,
        user_id: 1,
        unit_system: "mixed",
        created_at: created,
        updated_at: created,
      }),
      imported(1, 1109, "synthetic-1109.gpx", firstFinish),
      imported(2, 1110, "synthetic-1110.gpx", secondFinish),
      artifact(
        1,
        "synthetic-1109.gpx",
        "0548d650dd34cb5f453b0c85b3fc73c68b59d200b55a36a0fdb80b431923851b",
        firstFinish,
      ),
      artifact(
        2,
        "synthetic-1110.gpx",
        "63bd6ffdba13a352e690195e68abdb101c32aa95e172bec5223c3bd9d4f6605b",
        secondFinish,
      ),
      ride(
        1109,
        1,
        "Synthetic northbound A",
        "synthetic-1109.gpx",
        "2025-06-01T12:00:00Z",
        12,
      ),
      ride(
        1110,
        2,
        "Synthetic northbound B",
        "synthetic-1110.gpx",
        "2025-06-02T12:00:00Z",
        10,
      ),
      insert("segments", {
        id: 5,
        user_id: 1,
        title: "Synthetic northbound segment",
        source: "manual_upload",
        original_filename: "synthetic-1109.gpx",
        format: "gpx",
        distance_meters: 1000,
        route_data_json: points(12),
        mode: "xc",
        starred: false,
        created_at: firstFinish,
        updated_at: secondFinish,
        last_activity_change_at: secondFinish,
      }),
      effort(5895, 1109, 120, 2),
      effort(5912, 1110, 100, 1),
      insert("segment_summaries", {
        segment_id: 5,
        effort_count: 2,
        leader_user_id: 1,
        leader_effort_id: 5912,
        best_duration_seconds: 100,
        latest_activity_started_at: "2025-06-02T12:00:00Z",
        latest_activity_id: 1110,
        latest_effort_id: 5912,
        created_at: firstFinish,
        updated_at: secondFinish,
      }),
      insert("segment_user_summaries", {
        id: 1,
        segment_id: 5,
        user_id: 1,
        effort_count: 2,
        personal_best_effort_id: 5912,
        personal_best_duration_seconds: 100,
        created_at: firstFinish,
        updated_at: secondFinish,
      }),
    ],
  };
}

function activityData(id) {
  return baselineData().records.find(
    (record) => record.entity === "activities" && record.values.id === id,
  ).values.derived_data_json;
}

function archiveJobs() {
  return ["queued", "running", "succeeded", "failed"].map((status, index) => {
    const day = `2025-06-0${index + 1}`;
    const progressed = ["running", "succeeded"].includes(status);
    return insert("activity_archive_import_jobs", {
      id: 701 + index,
      user_id: 1,
      user_storage_key: "parity-user",
      archive_url: `https://example.invalid/exports/${status}.zip`,
      status,
      resolved_url: progressed
        ? `https://cdn.example.invalid/exports/${status}.zip`
        : null,
      failure_message:
        status === "failed" ? "Archive download returned HTTP 404" : null,
      error_samples_json: "[]",
      total_entries: progressed ? 10 : 0,
      supported_entry_count: progressed ? 8 : 0,
      imported_count: status === "succeeded" ? 6 : status === "running" ? 3 : 0,
      duplicate_count: progressed ? 1 : 0,
      skipped_unsupported_count: progressed ? 1 : 0,
      failed_count: status === "failed" ? 1 : 0,
      created_at: `${day}T12:00:00Z`,
      updated_at: `${day}T12:02:00Z`,
      started_at: status === "queued" ? null : `${day}T12:01:00Z`,
      finished_at: ["succeeded", "failed"].includes(status)
        ? `${day}T12:05:00Z`
        : null,
    });
  });
}

function lapScenario() {
  const derived = activityData(1109);
  derived.laps = ["Warmup", "Tempo"].map((title, index) => ({
    lap_index: index + 1,
    title,
    start_offset_seconds: index * 60,
    duration_seconds: 60,
    distance_meters: 500,
    average_speed_mps: 8.33,
    average_heart_rate_bpm: index === 0 ? 140 : 150,
    max_heart_rate_bpm: index === 0 ? 146 : 160,
  }));
  return [update("activities", { id: 1109, derived_data_json: derived })];
}

function climbScenario() {
  const derived = activityData(1109);
  derived.route_points = points(12, 10);
  derived.chart_points = points(12, 10);
  return [
    update("activities", {
      id: 1109,
      derived_data_json: derived,
      elevation_gain_meters: 100,
    }),
  ];
}

function zoneScenario() {
  const derived = activityData(1109);
  derived.chart_points = [];
  return [
    update("activities", {
      id: 1109,
      derived_data_json: derived,
      heart_rate_zones_json: [
        [1, null, 129, 30, 25000],
        [2, 130, 149, 90, 75000],
      ],
    }),
  ];
}

function elevenEffortScenario() {
  return [
    ...Array.from({ length: 9 }, (_, offset) => {
      const index = offset + 2;
      return effort(5998 + index, 1109, 120 + index, index + 1, index);
    }),
    update("segment_summaries", { segment_id: 5, effort_count: 11 }),
    update("segment_user_summaries", { id: 1, effort_count: 11 }),
  ];
}

function partialRaceScenario() {
  const derived = activityData(1110);
  derived.route_points = derived.route_points.filter((_, index) =>
    [0, 1, 4, 10].includes(index),
  );
  return [
    update("activities", { id: 1110, derived_data_json: derived }),
    update("segment_efforts", { id: 5912, end_route_point_index: 3 }),
  ];
}

function manualUserScenario() {
  return [
    [91002, "Segment Rebuild"],
    [91003, "XC Backfill"],
    [91004, "Import Reprocess"],
    [91005, "Duplicate Cleanup"],
  ].map(([id, name]) =>
    user(id, `parity-manual-${id}@bike.local`, `Parity ${name} User`),
  );
}

function uploadResultScenario() {
  return [
    imported(3, 1111, "synthetic-1109.gpx", "2025-06-05T12:02:00Z"),
    ride(
      1111,
      3,
      "Processed upload journey",
      "synthetic-1109.gpx",
      "2025-06-05T12:00:00Z",
      12,
    ),
  ];
}

const scenarioBuilders = {
  baseline: () => [],
  "representative-segment": () => segmentSceneData(baselineData()).records,
  laps: lapScenario,
  climb: climbScenario,
  zones: zoneScenario,
  "eleven-efforts": elevenEffortScenario,
  "partial-race": partialRaceScenario,
  "archive-states": archiveJobs,
  "manual-processing": manualUserScenario,
  "upload-result": uploadResultScenario,
  heatmap: () => [],
};

export function scenarioData(scenario) {
  if (!scenarios.includes(scenario))
    throw new Error(`Unknown E2E scenario: ${scenario}`);
  return {
    records: scenarioBuilders[scenario](),
    prepare_heatmaps: scenario === "heatmap",
  };
}

export function sessionData(token, userId = 1) {
  return {
    records: [
      insert("refresh_tokens", {
        token,
        user_pid: `00000000-0000-4000-8000-${String(userId).padStart(12, "0")}`,
        expires_at: Math.floor(Date.now() / 1000) + 86400,
        created_at: new Date().toISOString(),
      }),
    ],
  };
}

// Staging uses the same scenario records and the operator's normal SSO identity.
export function stagingData(ownerEmail, makeKey) {
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(ownerEmail)) {
    throw new Error("Staging requires the operator's login email");
  }
  const data = baselineData();
  for (const record of data.records) {
    if (record.entity !== "users") continue;
    record.values.api_key = makeKey(record.values.id);
    if (record.values.id === 1) {
      record.values.email = ownerEmail;
      record.values.name = "Staging Reviewer";
    }
  }
  return { ...data, prepare_heatmaps: true };
}

if (process.argv[2] === "--staging") {
  const { createHmac } = await import("node:crypto");
  const key = process.env.BIKE_STAGING_FIXTURE_KEY;
  if (!key || key.length < 32)
    throw new Error("Staging requires a private fixture key");
  const makeKey = (id) =>
    createHmac("sha256", key).update(String(id)).digest("hex");
  process.stdout.write(JSON.stringify(stagingData(process.argv[3], makeKey)));
}

export function heatmapPolicyData() {
  const derived = activityData(1109);
  for (const point of derived.route_points) {
    point.longitude -= 20;
    point.latitude += 3;
  }
  const variants = [
    { user_id: 2 },
    { sport: "run" },
    { format: "unavailable" },
    {
      derived_data_json: { ...derived, recording: { environment: "virtual" } },
    },
    { sport: "Virtual Ride" },
    { source: "trainer_road" },
    { title: "Zwift excluded recording" },
  ];
  return {
    records: variants.map((options, index) =>
      ride(
        1200 + index,
        null,
        "Heatmap policy fixture",
        "synthetic-1109.gpx",
        `2025-07-0${index + 1}T12:00:00Z`,
        12,
        { derived_data_json: derived, ...options },
      ),
    ),
    prepare_heatmaps: true,
  };
}

export function heatmapPaginationData() {
  // 65 disconnected paths in one z14 tile force a full 64-row page plus a tail.
  // Only the tail paints the lower line, so its pixels prove it was fetched.
  const route = Array.from({ length: 130 }, (_, index) => {
    const segment = Math.floor(index / 2);
    const x = (4000 + (index % 2 === 0 ? 0.2 : 0.8)) / 16384;
    const y = (6000 + (segment < 64 ? 0.25 : 0.75)) / 16384;
    return {
      ...points(12)[0],
      elapsed_seconds: segment * 240 + (index % 2) * 60,
      longitude: x * 360 - 180,
      latitude: (Math.atan(Math.sinh(Math.PI * (1 - 2 * y))) * 180) / Math.PI,
    };
  });
  return {
    records: [
      ride(
        1200,
        null,
        "Heatmap pagination fixture",
        "synthetic-1109.gpx",
        "2025-08-01T12:00:00Z",
        12,
        {
          sport: "mountain_bike",
          derived_data_json: { v: 2, route_points: route, chart_points: [] },
        },
      ),
    ],
    prepare_heatmaps: true,
  };
}
