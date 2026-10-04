-- Apply only to a disposable database cloned from four-views.sql.
-- Pass session_token with psql -v; use the same value for the selected test database.
BEGIN;

DO $$
BEGIN
    IF (SELECT count(*) FROM users WHERE id = 1) <> 1
       OR (SELECT count(*) FROM refresh_tokens) <> 0 THEN
        RAISE EXCEPTION 'session fixture requires a fresh four-view database';
    END IF;
END
$$;

INSERT INTO refresh_tokens (token, user_pid, expires_at, created_at)
SELECT :'session_token', pid,
       EXTRACT(EPOCH FROM CURRENT_TIMESTAMP + INTERVAL '1 day')::bigint,
       CURRENT_TIMESTAMP
FROM users WHERE id = 1;

COMMIT;
