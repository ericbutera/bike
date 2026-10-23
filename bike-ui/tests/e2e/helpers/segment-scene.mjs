import { readFileSync } from "node:fs";
import { Window } from "happy-dom";

// The original -g segment geometry, with deterministic disposable ride history.
const { DOMParser } = new Window();
const document = new DOMParser().parseFromString(
  readFileSync(new URL("../fixtures/segment-g.gpx", import.meta.url), "utf8"),
  "application/xml",
);
const coordinates = [...document.getElementsByTagName("trkpt")].map(
  (point) => ({
    latitude: Number(point.getAttribute("lat")),
    longitude: Number(point.getAttribute("lon")),
    elevation_meters: Number(point.getElementsByTagName("ele")[0].textContent),
  }),
);

function distanceBetween(left, right) {
  const radians = Math.PI / 180;
  const latitude = Math.sin(((right.latitude - left.latitude) * radians) / 2);
  const longitude = Math.sin(
    ((right.longitude - left.longitude) * radians) / 2,
  );
  const arc =
    latitude ** 2 +
    Math.cos(left.latitude * radians) *
      Math.cos(right.latitude * radians) *
      longitude ** 2;
  return 6_371_000 * 2 * Math.atan2(Math.sqrt(arc), Math.sqrt(1 - arc));
}

let distance = 0;
const route = coordinates.map((point, index) => {
  if (index > 0) distance += distanceBetween(coordinates[index - 1], point);
  return { ...point, distance_meters: distance };
});

export const segmentScene = {
  id: 16,
  title: "-g",
  effortCount: 37,
  selectedEffortIds: [7286, 7249, 6945],
};

const durations = [86, 89, 91, 95, 97, 97, 97, 98, 100, 100];
const firstDates = [
  "2026-08-11T18:36:00Z",
  "2026-08-08T16:44:00Z",
  "2026-08-10T14:25:00Z",
  "2026-08-12T14:48:00Z",
  "2025-06-08T17:16:00Z",
  "2026-05-26T14:51:00Z",
  "2026-08-15T11:13:00Z",
  "2025-06-11T18:24:00Z",
  "2025-07-14T21:43:00Z",
  "2026-08-17T13:19:00Z",
];
const attempts = Array.from(
  { length: segmentScene.effortCount },
  (_, index) => ({
    activityId: 30_000 + index,
    effortId: segmentScene.selectedEffortIds[index] ?? 9_000 + index,
    duration: durations[index] ?? 101 + (index - durations.length) * 3,
    startedAt:
      firstDates[index] ??
      (index === 15
        ? "2026-08-15T10:00:00Z"
        : `${2024 + (index % 3)}-06-${String(1 + (index % 27)).padStart(2, "0")}T12:00:00Z`),
  }),
);

function recording(attempt) {
  return route.map((point, index) => {
    const progress = index / (route.length - 1);
    const pace =
      progress +
      Math.sin(Math.PI * progress) * ((attempt.effortId % 3) - 1) * 0.025;
    return {
      ...point,
      elapsed_seconds: Math.round(attempt.duration * pace),
      speed_mps:
        (distance / attempt.duration) *
        (1 + Math.sin(progress * Math.PI * 3) * 0.2),
      heart_rate_bpm:
        145 +
        (attempt.effortId % 17) +
        Math.round(Math.sin(progress * Math.PI) * 8),
      cadence_rpm: 75 + Math.round(Math.sin(progress * Math.PI * 2) * 9),
    };
  });
}

function fixtureRecord(source, values) {
  return { ...source, values: { ...source.values, ...values } };
}

function activityRecords(source) {
  return attempts.map((attempt) => {
    const points = recording(attempt);
    return fixtureRecord(source, {
      id: attempt.activityId,
      activity_import_id: null,
      title: "Afternoon Mountain Bike Ride Copy",
      original_filename: "Segment - -g.gpx",
      started_at: attempt.startedAt,
      ended_at: new Date(
        Date.parse(attempt.startedAt) + attempt.duration * 1_000,
      ).toISOString(),
      distance_meters: distance,
      moving_time_seconds: attempt.duration,
      total_time_seconds: attempt.duration,
      average_speed_mps: distance / attempt.duration,
      max_speed_mps: Math.max(...points.map((point) => point.speed_mps)),
      derived_data_json: { v: 2, route_points: points, chart_points: points },
      created_at: attempt.startedAt,
      updated_at: attempt.startedAt,
    });
  });
}

function effortRecords(source) {
  return attempts.map((attempt, index) =>
    fixtureRecord(source, {
      id: attempt.effortId,
      segment_id: segmentScene.id,
      activity_id: attempt.activityId,
      effort_index: 1,
      start_route_point_index: 0,
      end_route_point_index: route.length - 1,
      start_elapsed_seconds: 0,
      end_elapsed_seconds: attempt.duration,
      duration_seconds: attempt.duration,
      distance_meters: distance,
      overall_rank: index + 1,
      user_rank: index + 1,
      created_at: attempt.startedAt,
      updated_at: attempt.startedAt,
    }),
  );
}

export function segmentSceneData(baseline) {
  const source = (entity) =>
    baseline.records.find((record) => record.entity === entity);
  const lastChange = "2026-08-17T13:19:00Z";
  return {
    records: [
      {
        ...fixtureRecord(source("user_preferences"), {
          unit_system: "imperial",
        }),
        operation: "update",
      },
      ...activityRecords(source("activities")),
      fixtureRecord(source("segments"), {
        id: segmentScene.id,
        title: segmentScene.title,
        source: "manual_segment_import",
        original_filename: "Segment - -g.gpx",
        distance_meters: distance,
        route_data_json: recording(attempts[0]),
        created_at: "2026-05-07T17:05:00Z",
        updated_at: lastChange,
        last_activity_change_at: lastChange,
      }),
      ...effortRecords(source("segment_efforts")),
      fixtureRecord(source("segment_summaries"), {
        segment_id: segmentScene.id,
        effort_count: segmentScene.effortCount,
        leader_effort_id: attempts[0].effortId,
        best_duration_seconds: attempts[0].duration,
        latest_activity_started_at: lastChange,
        latest_activity_id: attempts[9].activityId,
        latest_effort_id: attempts[9].effortId,
        updated_at: lastChange,
      }),
      fixtureRecord(source("segment_user_summaries"), {
        id: segmentScene.id,
        segment_id: segmentScene.id,
        effort_count: segmentScene.effortCount,
        personal_best_effort_id: attempts[0].effortId,
        personal_best_duration_seconds: attempts[0].duration,
        updated_at: lastChange,
      }),
    ],
  };
}
