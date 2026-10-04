-- Add long-ride and fitness evidence to a separate copy of four-views.sql.
-- This must never be loaded into the product or four-view screenshot database.
BEGIN;

DO $$
BEGIN
    IF (SELECT count(*) FROM activities) <> 2
       OR EXISTS (SELECT 1 FROM activities WHERE id IN (2001, 2002))
       OR (SELECT count(*) FROM users) <> 1 THEN
        RAISE EXCEPTION 'reassessment fixture requires a fresh four-view database';
    END IF;
END
$$;

UPDATE user_preferences
SET xc_goal_start_date = '2026-06-01',
    xc_goal_target_date = '2026-10-15',
    xc_goal_event_name = 'Synthetic XC century',
    xc_goal_target_distance_meters = 100000,
    xc_goal_target_elevation_gain_meters = 1500,
    xc_goal_target_finish_time_seconds = 21600,
    xc_goal_event_profile = 'xc_marathon',
    updated_at = '2026-06-01T00:00:00Z'
WHERE user_id = 1;

INSERT INTO activities
    (id, user_id, title, sport, source, format, activity_type, started_at,
     ended_at, distance_meters, moving_time_seconds, total_time_seconds,
     elevation_gain_meters, elevation_loss_meters, average_speed_mps,
     derived_data_json, created_at, updated_at)
VALUES
    (2001, 1, 'Synthetic spring long ride', 'ride', 'manual_upload', 'gpx',
     'training', '2026-04-15T12:00:00Z', '2026-04-15T16:00:00Z',
     60000, 13500, 14400, 900, 900, 60000.0 / 13500,
     '{"v":2,"route_points":[],"chart_points":[]}',
     '2026-04-15T16:00:00Z', '2026-04-15T16:00:00Z'),
    (2002, 1, 'Synthetic current long ride', 'ride', 'manual_upload', 'gpx',
     'training', '2026-09-15T12:00:00Z', '2026-09-15T16:10:00Z',
     75000, 14000, 15000, 1200, 1200, 75000.0 / 14000,
     '{"v":2,"route_points":[],"chart_points":[]}',
     '2026-09-15T16:10:00Z', '2026-09-15T16:10:00Z');

INSERT INTO fitness_freshness_daily
    (user_id, day, activity_count, training_load, fitness, fatigue, form,
     created_at, updated_at)
VALUES
    (1, '2026-04-14', 0, 0, 38, 36, 2,
     '2026-04-14T23:59:59Z', '2026-04-14T23:59:59Z'),
    (1, '2026-04-15', 1, 100, 40, 48, -8,
     '2026-04-15T23:59:59Z', '2026-04-15T23:59:59Z'),
    (1, '2026-09-14', 0, 0, 53, 51, 2,
     '2026-09-14T23:59:59Z', '2026-09-14T23:59:59Z'),
    (1, '2026-09-15', 1, 120, 55, 64, -9,
     '2026-09-15T23:59:59Z', '2026-09-15T23:59:59Z');

SELECT setval(pg_get_serial_sequence('activities', 'id'), 2002, true);
COMMIT;
