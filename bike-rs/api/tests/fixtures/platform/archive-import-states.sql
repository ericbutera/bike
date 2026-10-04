-- Apply only to the disposable bike_test fixture, then recreate that fixture.
-- Exercise every visible archive-import state and its progress fields.
BEGIN;

DO $$
BEGIN
    IF (SELECT COUNT(*) FROM users) <> 3
       OR EXISTS (SELECT 1 FROM activity_archive_import_jobs)
       OR EXISTS (SELECT 1 FROM activity_import_locks) THEN
        RAISE EXCEPTION 'archive state fixture requires a fresh four-view database';
    END IF;
END
$$;

INSERT INTO activity_archive_import_jobs
    (id, user_id, user_storage_key, archive_url, resolved_url, status,
     failure_message, error_samples_json, total_entries, supported_entry_count,
     imported_count, duplicate_count, skipped_unsupported_count, failed_count,
     created_at, started_at, finished_at, updated_at)
VALUES
    (701, 1, 'parity-user', 'https://example.invalid/exports/queued.zip', NULL,
     'queued', NULL, '[]', 0, 0, 0, 0, 0, 0,
     '2025-06-01T12:00:00Z', NULL, NULL, '2025-06-01T12:00:00Z'),
    (702, 1, 'parity-user', 'https://example.invalid/exports/running.zip',
     'https://cdn.example.invalid/exports/running.zip', 'running', NULL, '[]',
     10, 8, 3, 1, 1, 0,
     '2025-06-02T12:00:00Z', '2025-06-02T12:01:00Z', NULL,
     '2025-06-02T12:02:00Z'),
    (703, 1, 'parity-user', 'https://example.invalid/exports/succeeded.zip',
     'https://cdn.example.invalid/exports/succeeded.zip', 'succeeded', NULL, '[]',
     10, 8, 6, 1, 1, 0,
     '2025-06-03T12:00:00Z', '2025-06-03T12:01:00Z', '2025-06-03T12:05:00Z',
     '2025-06-03T12:05:00Z'),
    (704, 1, 'parity-user', 'https://example.invalid/exports/failed.zip', NULL,
     'failed', 'Archive download returned HTTP 404', '[]', 0, 0, 0, 0, 0, 1,
     '2025-06-04T12:00:00Z', '2025-06-04T12:01:00Z', '2025-06-04T12:02:00Z',
     '2025-06-04T12:02:00Z');

SELECT setval(pg_get_serial_sequence('activity_archive_import_jobs', 'id'), 704, true);

COMMIT;
