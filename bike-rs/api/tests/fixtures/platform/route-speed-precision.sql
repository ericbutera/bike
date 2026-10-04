-- Keep fixture speed values at three decimal places so each backend parses
-- identical JSON numbers when rendering activity charts.
UPDATE activities
SET derived_data_json = jsonb_set(
    jsonb_set(
        derived_data_json::jsonb,
        '{route_points}',
        (
            SELECT jsonb_agg(
                point || jsonb_build_object(
                    'speed_mps',
                    round((point->>'speed_mps')::numeric, 3)
                )
                ORDER BY ordinal
            )
            FROM jsonb_array_elements(derived_data_json::jsonb->'route_points')
                 WITH ORDINALITY AS route(point, ordinal)
        )
    ),
    '{chart_points}',
    (
        SELECT jsonb_agg(
            point || jsonb_build_object(
                'speed_mps',
                round((point->>'speed_mps')::numeric, 3)
            )
            ORDER BY ordinal
        )
        FROM jsonb_array_elements(derived_data_json::jsonb->'chart_points')
             WITH ORDINALITY AS chart(point, ordinal)
    )
)
WHERE id IN (1109, 1110)
  AND derived_data_json IS NOT NULL;
