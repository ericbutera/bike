-- Apply only to the disposable bike_test fixture, then recreate that fixture.
-- Keep one compared effort sparsely sampled to exercise interpolation and live gaps.
BEGIN;

DO $$
BEGIN
    IF (SELECT COUNT(*) FROM segment_efforts WHERE id IN (5895, 5912)) <> 2
       OR (SELECT jsonb_array_length(derived_data_json::jsonb->'route_points') FROM activities WHERE id = 1110) <> 11 THEN
        RAISE EXCEPTION 'partial race fixture requires the pinned two-effort activity fixture';
    END IF;
END
$$;

UPDATE activities
SET derived_data_json = jsonb_set(
    derived_data_json::jsonb,
    '{route_points}',
    (
        SELECT jsonb_agg(point ORDER BY ordinality)
        FROM jsonb_array_elements(derived_data_json::jsonb->'route_points') WITH ORDINALITY AS samples(point, ordinality)
        WHERE ordinality IN (1, 2, 5, 11)
    )
)::json
WHERE id = 1110;

UPDATE segment_efforts
SET end_route_point_index = 3
WHERE id = 5912;

COMMIT;
