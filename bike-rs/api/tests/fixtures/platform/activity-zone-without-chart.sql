-- Apply only to the disposable bike_test fixture, then recreate that fixture.
-- Preserve the route and zone rollup while removing the signal chart samples.
BEGIN;

DO $$
BEGIN
    IF (SELECT COUNT(*) FROM activities WHERE id IN (1109, 1110)) <> 2
       OR (SELECT heart_rate_zones_json IS NOT NULL FROM activities WHERE id = 1109) THEN
        RAISE EXCEPTION 'zone fixture requires the unchanged two-activity parity fixture';
    END IF;
END
$$;

UPDATE activities
SET derived_data_json = jsonb_set(derived_data_json::jsonb, '{chart_points}', '[]'::jsonb)::json,
    heart_rate_zones_json = '[[1,null,129,30,25000],[2,130,149,90,75000]]'::json
WHERE id = 1109;

COMMIT;
