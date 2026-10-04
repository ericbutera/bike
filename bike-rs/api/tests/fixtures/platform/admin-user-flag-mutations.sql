INSERT INTO users (id, pid, email, api_key, name, is_admin, email_verified_at, created_at, updated_at)
VALUES (91012, '91012000-0000-4000-8000-000000000012', 'parity-admin-91012@bike.local',
        'parity-admin-91012', 'Parity Admin Target', false,
        '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')
ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, is_admin = EXCLUDED.is_admin;

INSERT INTO users (id, pid, email, api_key, name, is_admin, email_verified_at, created_at, updated_at)
SELECT id, (id::text || '000-0000-4000-8000-0000000' || id::text)::uuid,
       'parity-admin-' || id::text || '@bike.local', 'parity-admin-' || id::text,
       'Parity Admin Target ' || id::text, false,
       '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z'
FROM generate_series(91013, 91015) AS ids(id)
ON CONFLICT (id) DO NOTHING;

INSERT INTO feature_flags (feature_key, enabled, description, created_at, updated_at)
VALUES ('story16-admin-flag', false, 'Story 16 parity flag',
        '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')
ON CONFLICT (feature_key) DO UPDATE
SET enabled = EXCLUDED.enabled, description = EXCLUDED.description;
