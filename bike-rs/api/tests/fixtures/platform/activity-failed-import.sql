-- Apply only to the disposable bike_test fixture, then recreate that fixture.
BEGIN;

DO $$
BEGIN
    IF (SELECT COUNT(*) FROM activity_imports) <> 2 THEN
        RAISE EXCEPTION 'failed-import fixture requires the two-import parity fixture';
    END IF;
END
$$;

INSERT INTO activity_imports
    (id, user_id, source, format, status, original_filename, storage_path,
     size_bytes, mime_type, processing_stage, processing_attempts,
     processing_error, created_at, updated_at)
VALUES
    (3, 1, 'manual_upload', 'gpx', 'failed', 'failed-parity.gpx',
     'activity-imports/failed-parity.gpx', 42, 'application/gpx+xml',
     'raw_stored', 1, 'synthetic parse failure',
     '2025-06-03T12:00:00Z', '2025-06-03T12:01:00Z');

SELECT setval(pg_get_serial_sequence('activity_imports', 'id'), 3, true);

COMMIT;
