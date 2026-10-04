-- Apply only to the disposable bike_test fixture, then recreate that fixture.
BEGIN;

DO $$
BEGIN
    IF (SELECT status FROM background_tasks WHERE id = 5) <> 'pending' THEN
        RAISE EXCEPTION 'processing-task fixture requires pending task 5';
    END IF;
END
$$;

UPDATE background_tasks
SET status = 'processing', attempts = 1,
    started_at = '2025-06-07T12:01:00Z', updated_at = '2025-06-07T12:01:00Z'
WHERE id = 5;

COMMIT;
