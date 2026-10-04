-- Apply only to the disposable bike_test fixture, then recreate that fixture.
-- Repeated attempts on one ride exercise the effort list's second page.
BEGIN;

DO $$
BEGIN
    IF (SELECT COUNT(*) FROM segment_efforts WHERE segment_id = 5) <> 2
       OR (SELECT COUNT(*) FROM activities WHERE id IN (1109, 1110)) <> 2 THEN
        RAISE EXCEPTION 'eleven-effort fixture requires the unchanged two-activity parity fixture';
    END IF;
END
$$;

INSERT INTO segment_efforts
    (id, user_id, segment_id, activity_id, effort_index,
     start_route_point_index, end_route_point_index, start_elapsed_seconds,
     end_elapsed_seconds, duration_seconds, distance_meters, overall_rank,
     user_rank, created_at, updated_at)
SELECT 5998 + attempt, 1, 5, 1109, attempt, 0, 10, 0,
       120 + attempt, 120 + attempt, 1000, attempt + 1,
       attempt + 1, '2025-06-01T12:02:00Z', '2025-06-02T12:01:40Z'
FROM generate_series(2, 10) AS attempt;

UPDATE segment_summaries SET effort_count = 11 WHERE segment_id = 5;
UPDATE segment_user_summaries SET effort_count = 11 WHERE segment_id = 5 AND user_id = 1;
SELECT setval(pg_get_serial_sequence('segment_efforts', 'id'), 6008, true);

COMMIT;
