-- Apply only to the disposable bike_test fixture, then recreate that fixture.
BEGIN;

DO $$
BEGIN
    IF (SELECT COUNT(*) FROM activities WHERE id IN (1109, 1110)) <> 2
       OR (SELECT derived_data_json::jsonb ? 'laps' FROM activities WHERE id = 1109) THEN
        RAISE EXCEPTION 'lap fixture requires the unchanged two-activity parity fixture';
    END IF;
END
$$;

UPDATE activities
SET derived_data_json = jsonb_set(derived_data_json::jsonb, '{laps}',
        '[{"lap_index":1,"title":"Warmup","start_offset_seconds":0,"duration_seconds":60,"distance_meters":500,"average_speed_mps":8.33,"average_heart_rate_bpm":140,"max_heart_rate_bpm":146},
          {"lap_index":2,"title":"Tempo","start_offset_seconds":60,"duration_seconds":60,"distance_meters":500,"average_speed_mps":8.33,"average_heart_rate_bpm":150,"max_heart_rate_bpm":160}]'::jsonb)::json
WHERE id = 1109;

COMMIT;
