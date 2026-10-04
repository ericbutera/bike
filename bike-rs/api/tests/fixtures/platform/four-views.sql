-- Synthetic comparison data for activity list/detail and segment detail/race.
-- Load only into a fresh, migrated test database. The guard prevents this
-- fixture from changing a developer's existing activities or users.
BEGIN;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM users)
       OR EXISTS (SELECT 1 FROM activities)
       OR EXISTS (SELECT 1 FROM segments)
       OR EXISTS (SELECT 1 FROM segment_efforts) THEN
        RAISE EXCEPTION 'four-views fixture requires an empty test database';
    END IF;
END
$$;

INSERT INTO users (id, pid, email, api_key, name, is_admin, email_verified_at, created_at, updated_at)
VALUES (1, '00000000-0000-4000-8000-000000000001', 'developer@bike.local',
        'bike-test-local-admin', 'Local Developer', true,
        '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z');

INSERT INTO user_preferences (id, user_id, unit_system, created_at, updated_at)
VALUES (1, 1, 'mixed', '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z');

INSERT INTO activity_imports
    (id, user_id, source, format, status, original_filename, storage_path,
     size_bytes, mime_type, activity_id, processing_stage, processing_attempts,
     processed_at, created_at, updated_at)
VALUES
    (1, 1, 'manual_upload', 'gpx', 'processed', 'synthetic-1109.gpx',
     'activity-imports/synthetic-1109.gpx', 1268, 'application/gpx+xml', 1109,
     'complete', 1, '2025-06-01T12:02:00Z', '2025-06-01T12:02:00Z', '2025-06-01T12:02:00Z'),
    (2, 1, 'manual_upload', 'gpx', 'processed', 'synthetic-1110.gpx',
     'activity-imports/synthetic-1110.gpx', 1268, 'application/gpx+xml', 1110,
     'complete', 1, '2025-06-02T12:01:40Z', '2025-06-02T12:01:40Z', '2025-06-02T12:01:40Z');

INSERT INTO activity_import_artifacts
    (id, activity_import_id, user_id, artifact_kind, format, source_quality,
     original_filename, storage_path, size_bytes, mime_type, checksum_sha256,
     created_at, updated_at)
VALUES
    (1, 1, 1, 'original', 'gpx', 'gpx_original', 'synthetic-1109.gpx',
     'activity-imports/synthetic-1109.gpx', 1268, 'application/gpx+xml',
     '0548d650dd34cb5f453b0c85b3fc73c68b59d200b55a36a0fdb80b431923851b',
     '2025-06-01T12:02:00Z', '2025-06-01T12:02:00Z'),
    (2, 2, 1, 'original', 'gpx', 'gpx_original', 'synthetic-1110.gpx',
     'activity-imports/synthetic-1110.gpx', 1268, 'application/gpx+xml',
     '63bd6ffdba13a352e690195e68abdb101c32aa95e172bec5223c3bd9d4f6605b',
     '2025-06-02T12:01:40Z', '2025-06-02T12:01:40Z');

WITH rides(id, import_id, title, filename, started_at, step_seconds) AS (
    VALUES
        (1109, 1, 'Synthetic northbound A', 'synthetic-1109.gpx', '2025-06-01T12:00:00Z'::timestamptz, 12),
        (1110, 2, 'Synthetic northbound B', 'synthetic-1110.gpx', '2025-06-02T12:00:00Z'::timestamptz, 10)
)
INSERT INTO activities
    (id, user_id, activity_import_id, title, sport, source, original_filename,
     format, activity_type, started_at, ended_at, distance_meters,
     moving_time_seconds, total_time_seconds, elevation_gain_meters,
     elevation_loss_meters, average_speed_mps, max_speed_mps,
     average_heart_rate_bpm, max_heart_rate_bpm, calories, derived_data_json,
     created_at, updated_at)
SELECT id, 1, import_id, title, 'ride', 'manual_upload', filename, 'gpx',
       'training', started_at, started_at + make_interval(secs => step_seconds * 10),
       1000, step_seconds * 10, step_seconds * 10, 20, 0,
       1000.0 / (step_seconds * 10), 12, 145, 160, 100,
       json_build_object(
           'v', 2,
           'route_points', (
               SELECT json_agg(json_build_object(
                   'elapsed_seconds', point_index * step_seconds,
                   'latitude', 42.0 + point_index * 0.001,
                   'longitude', -83.0,
                   'distance_meters', point_index * 100.0,
                   'elevation_meters', 200.0 + point_index * 2.0,
                   'speed_mps', ROUND(100.0 / step_seconds, 3),
                   'heart_rate_bpm', 140 + point_index
               ) ORDER BY point_index)
               FROM generate_series(0, 10) AS point_index
           ),
           'chart_points', (
               SELECT json_agg(json_build_object(
                   'elapsed_seconds', point_index * step_seconds,
                   'distance_meters', point_index * 100.0,
                   'elevation_meters', 200.0 + point_index * 2.0,
                   'speed_mps', 100.0 / step_seconds,
                   'heart_rate_bpm', 140 + point_index
               ) ORDER BY point_index)
               FROM generate_series(0, 10) AS point_index
           )
       ), started_at, started_at
FROM rides;

INSERT INTO segments
    (id, user_id, title, source, original_filename, format, distance_meters,
     route_data_json, mode, starred, created_at, updated_at, last_activity_change_at)
VALUES
    (5, 1, 'Synthetic northbound segment', 'manual_upload', 'synthetic-1109.gpx',
     'gpx', 1000,
     (SELECT json_agg(json_build_object(
         'elapsed_seconds', point_index * 12,
         'latitude', 42.0 + point_index * 0.001,
         'longitude', -83.0,
         'distance_meters', point_index * 100.0,
         'elevation_meters', 200.0 + point_index * 2.0
     ) ORDER BY point_index) FROM generate_series(0, 10) AS point_index),
     'xc', false, '2025-06-01T12:02:00Z', '2025-06-01T12:02:00Z', '2025-06-02T12:01:40Z');

INSERT INTO segment_efforts
    (id, user_id, segment_id, activity_id, effort_index,
     start_route_point_index, end_route_point_index, start_elapsed_seconds,
     end_elapsed_seconds, duration_seconds, distance_meters, overall_rank,
     user_rank, created_at, updated_at)
VALUES
    (5895, 1, 5, 1109, 1, 0, 10, 0, 120, 120, 1000, 2, 2,
     '2025-06-01T12:02:00Z', '2025-06-02T12:01:40Z'),
    (5912, 1, 5, 1110, 1, 0, 10, 0, 100, 100, 1000, 1, 1,
     '2025-06-02T12:01:40Z', '2025-06-02T12:01:40Z');

INSERT INTO segment_summaries
    (segment_id, effort_count, leader_user_id, leader_effort_id,
     best_duration_seconds, latest_activity_started_at, latest_activity_id,
     latest_effort_id, created_at, updated_at)
VALUES
    (5, 2, 1, 5912, 100, '2025-06-02T12:00:00Z', 1110, 5912,
     '2025-06-01T12:02:00Z', '2025-06-02T12:01:40Z');

INSERT INTO segment_user_summaries
    (id, segment_id, user_id, effort_count, personal_best_effort_id,
     personal_best_duration_seconds, created_at, updated_at)
VALUES
    (1, 5, 1, 2, 5912, 100, '2025-06-01T12:02:00Z', '2025-06-02T12:01:40Z');

-- Five durable jobs cover task filtering, timing/trend, detail, and available
-- actions. The pending job is scheduled far ahead so a fixture worker cannot
-- consume it during a read-only test run.
INSERT INTO background_tasks
    (id, task_type, payload, status, attempts, max_attempts, error, result,
     scheduled_for, started_at, completed_at, created_at, updated_at)
VALUES
    (1, 'regenerate_segment_efforts', '{"segment_id":5}', 'completed', 1, 3, NULL, '2 efforts',
     '2025-06-03T12:00:00Z', '2025-06-03T12:00:00Z', '2025-06-03T12:00:50Z',
     '2025-06-03T12:00:00Z', '2025-06-03T12:00:50Z'),
    (2, 'regenerate_segment_efforts', '{"segment_id":5}', 'completed', 1, 3, NULL, '2 efforts',
     '2025-06-04T12:00:00Z', '2025-06-04T12:00:00Z', '2025-06-04T12:00:55Z',
     '2025-06-04T12:00:00Z', '2025-06-04T12:00:55Z'),
    (3, 'regenerate_segment_efforts', '{"segment_id":5}', 'completed', 1, 3, NULL, '2 efforts',
     '2025-06-05T12:00:00Z', '2025-06-05T12:00:00Z', '2025-06-05T12:01:52Z',
     '2025-06-05T12:00:00Z', '2025-06-05T12:01:52Z'),
    (4, 'rebuild_fitness_freshness', '{"user_id":1}', 'failed', 1, 3, 'synthetic worker failure', NULL,
     '2025-06-06T12:00:00Z', '2025-06-06T12:00:00Z', '2025-06-06T12:00:01Z',
     '2025-06-06T12:00:00Z', '2025-06-06T12:00:01Z'),
    (5, 'process_activity_import', '{"import_id":1,"user_id":1}', 'pending', 0, 3, NULL, NULL,
     '2099-01-01T00:00:00Z', NULL, NULL,
     '2025-06-07T12:00:00Z', '2025-06-07T12:00:00Z');

SELECT setval(pg_get_serial_sequence('users', 'id'), 1, true);
SELECT setval(pg_get_serial_sequence('user_preferences', 'id'), 1, true);
SELECT setval(pg_get_serial_sequence('activities', 'id'), 1110, true);
SELECT setval(pg_get_serial_sequence('activity_imports', 'id'), 2, true);
SELECT setval(pg_get_serial_sequence('activity_import_artifacts', 'id'), 2, true);
SELECT setval(pg_get_serial_sequence('segments', 'id'), 5, true);
SELECT setval(pg_get_serial_sequence('segment_efforts', 'id'), 5912, true);
SELECT setval(pg_get_serial_sequence('segment_user_summaries', 'id'), 1, true);
SELECT setval(pg_get_serial_sequence('background_tasks', 'id'), 5, true);

COMMIT;
