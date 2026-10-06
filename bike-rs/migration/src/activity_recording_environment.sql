-- A stored summary avoids repeatedly detoasting and parsing the entire GPS
-- payload in heatmap read predicates. PostgreSQL derives it atomically on every
-- insert/update; application writers and older workers cannot desynchronize it.
ALTER TABLE activities ADD COLUMN recording_environment text
    GENERATED ALWAYS AS
    (coalesce(derived_data_json->'recording'->>'environment', 'unknown')) STORED;
