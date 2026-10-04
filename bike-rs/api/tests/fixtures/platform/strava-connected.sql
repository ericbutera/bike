DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM strava_connections WHERE user_id = 1) THEN
        RAISE EXCEPTION 'Strava connection fixture already exists';
    END IF;
END $$;

INSERT INTO strava_connections
    (id, user_id, athlete_id, athlete_username, athlete_first_name,
     athlete_last_name, scopes, access_token, refresh_token, expires_at,
     last_sync_status)
VALUES
    (9501, 1, 95551, 'story16-rider', 'Story', 'Rider',
     'read,activity:read_all', 'story16-access', 'story16-refresh',
     '2099-01-01T00:00:00Z', 'never');
