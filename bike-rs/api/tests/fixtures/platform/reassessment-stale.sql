-- Apply only to a disposable bike_reassessment database after reassessment.sql.
-- The spring ride becomes last-known evidence; no current long ride remains.
BEGIN;

DO $$
BEGIN
    IF (SELECT count(*) FROM activities WHERE id IN (2001, 2002)) <> 2
       OR (SELECT xc_goal_start_date FROM user_preferences WHERE user_id = 1) <> '2026-06-01'::date THEN
        RAISE EXCEPTION 'stale reassessment fixture requires a fresh benchmark overlay';
    END IF;
END
$$;

DELETE FROM activities WHERE id = 2002;
UPDATE user_preferences SET xc_goal_start_date = '2026-04-01' WHERE user_id = 1;

COMMIT;
