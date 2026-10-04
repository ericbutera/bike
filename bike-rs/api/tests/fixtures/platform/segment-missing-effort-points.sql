-- Apply only to the disposable bike_test fixture, then recreate that fixture.
-- The faster effort retains its metadata but loses its stored point series.
BEGIN;

DO $$
BEGIN
    IF (SELECT COUNT(*) FROM segment_efforts WHERE segment_id = 5 AND id IN (5895, 5912)) <> 2 THEN
        RAISE EXCEPTION 'missing-point fixture requires the pinned segment efforts';
    END IF;
END
$$;

UPDATE activities
SET derived_data_json = jsonb_set(
        jsonb_set(derived_data_json::jsonb, '{route_points}', '[]'::jsonb),
        '{chart_points}', '[]'::jsonb)::json
WHERE id = 1110;

COMMIT;
