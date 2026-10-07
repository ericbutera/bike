-- Stable active/disabled users for the admin-users list and account controls.
-- Safe to reapply to the isolated test database.
BEGIN;

INSERT INTO users
    (id, pid, email, api_key, name, is_admin, disabled, email_verified_at, created_at, updated_at)
VALUES
    (2, '00000000-0000-4000-8000-000000000002', 'active-rider@bike.local',
     'bike-test-active-rider', 'Active Parity Rider', false, false,
     '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z', '2025-01-02T00:00:00Z'),
    (3, '00000000-0000-4000-8000-000000000003', 'disabled-rider@bike.local',
     'bike-test-disabled-rider', 'Disabled Parity Rider', false, true,
     NULL, '2025-01-03T00:00:00Z', '2025-01-03T00:00:00Z')
ON CONFLICT (id) DO UPDATE SET
    pid = EXCLUDED.pid,
    email = EXCLUDED.email,
    api_key = EXCLUDED.api_key,
    name = EXCLUDED.name,
    is_admin = EXCLUDED.is_admin,
    disabled = EXCLUDED.disabled,
    email_verified_at = EXCLUDED.email_verified_at,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at;

SELECT setval(pg_get_serial_sequence('users', 'id'), GREATEST((SELECT MAX(id) FROM users), 1), true);

COMMIT;
