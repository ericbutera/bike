-- Synthetic data for the native HTTP integration test. SQLite only.
INSERT INTO activities
    (id, user_id, title, sport, source, activity_type, started_at,
     distance_meters, moving_time_seconds, derived_data_json, created_at, updated_at)
VALUES
    (1109, 1, 'Synthetic northbound A', 'ride', 'manual_upload', 'training',
     '2025-06-01T12:00:00Z', 1000, 120,
     '{"v":2,"route_points":[{"elapsed_seconds":0,"latitude":42.0,"longitude":-83.0,"distance_meters":0},{"elapsed_seconds":120,"latitude":42.01,"longitude":-83.0,"distance_meters":1000}]}',
     '2025-06-01T12:00:00Z', '2025-06-01T12:00:00Z'),
    (1110, 1, 'Synthetic northbound B', 'ride', 'manual_upload', 'training',
     '2025-06-02T12:00:00Z', 1000, 100,
     '{"v":2,"route_points":[{"elapsed_seconds":0,"latitude":42.0,"longitude":-83.0,"distance_meters":0},{"elapsed_seconds":100,"latitude":42.01,"longitude":-83.0,"distance_meters":1000}]}',
     '2025-06-02T12:00:00Z', '2025-06-02T12:00:00Z');

INSERT INTO segments
    (id, user_id, title, source, mode, distance_meters, starred, route_data_json,
     last_activity_change_at, created_at, updated_at)
VALUES
    (5, 1, 'Synthetic northbound segment', 'manual_upload', 'xc', 1000, false,
     '[{"elapsed_seconds":0,"latitude":42.0,"longitude":-83.0,"distance_meters":0},{"elapsed_seconds":120,"latitude":42.01,"longitude":-83.0,"distance_meters":1000}]',
     '2025-06-02T12:00:00Z', '2025-06-01T12:00:00Z', '2025-06-02T12:00:00Z');

INSERT INTO segment_efforts
    (id, user_id, segment_id, activity_id, effort_index,
     start_route_point_index, end_route_point_index, start_elapsed_seconds,
     end_elapsed_seconds, duration_seconds, distance_meters, overall_rank,
     user_rank, created_at, updated_at)
VALUES
    (5895, 1, 5, 1109, 1, 0, 1, 0, 120, 120, 1000, 2, 2,
     '2025-06-01T12:00:00Z', '2025-06-02T12:00:00Z'),
    (5912, 1, 5, 1110, 1, 0, 1, 0, 100, 100, 1000, 1, 1,
     '2025-06-02T12:00:00Z', '2025-06-02T12:00:00Z');

INSERT INTO segment_summaries
    (segment_id, effort_count, leader_user_id, leader_effort_id,
     best_duration_seconds, latest_activity_started_at, latest_activity_id,
     latest_effort_id, created_at, updated_at)
VALUES
    (5, 2, 1, 5912, 100, '2025-06-02T12:00:00Z', 1110, 5912,
     '2025-06-01T12:00:00Z', '2025-06-02T12:00:00Z');

INSERT INTO segment_user_summaries
    (id, segment_id, user_id, effort_count, personal_best_effort_id,
     personal_best_duration_seconds, created_at, updated_at)
VALUES
    (1, 5, 1, 2, 5912, 100, '2025-06-01T12:00:00Z', '2025-06-02T12:00:00Z');
