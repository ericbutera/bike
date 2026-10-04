-- Apply only to the disposable bike_test fixture, then recreate that fixture.
-- The route gains 100 meters over 1,000 meters in 120 seconds: one sustained climb.
BEGIN;

DO $$
BEGIN
    IF (SELECT COUNT(*) FROM activities WHERE id IN (1109, 1110)) <> 2
       OR (SELECT COUNT(*) FROM segment_efforts WHERE segment_id = 5) <> 2 THEN
        RAISE EXCEPTION 'climb route requires the unchanged two-activity parity fixture';
    END IF;
END
$$;

WITH raised AS (
    SELECT id,
           (SELECT jsonb_agg(jsonb_set(point, '{elevation_meters}',
                       to_jsonb(200 + (ordinal - 1) * 10)) ORDER BY ordinal)
            FROM jsonb_array_elements(derived_data_json::jsonb -> 'route_points')
                 WITH ORDINALITY AS route(point, ordinal)) AS route_points,
           (SELECT jsonb_agg(jsonb_set(point, '{elevation_meters}',
                       to_jsonb(200 + (ordinal - 1) * 10)) ORDER BY ordinal)
            FROM jsonb_array_elements(derived_data_json::jsonb -> 'chart_points')
                 WITH ORDINALITY AS chart(point, ordinal)) AS chart_points
    FROM activities WHERE id = 1109
)
UPDATE activities AS activity
SET derived_data_json = jsonb_set(
        jsonb_set(activity.derived_data_json::jsonb, '{route_points}', raised.route_points),
        '{chart_points}', raised.chart_points)::json,
    elevation_gain_meters = 100
FROM raised WHERE activity.id = raised.id;

COMMIT;
