INSERT INTO users (id, pid, email, api_key, name, is_admin, email_verified_at)
VALUES
  (91002, '91002000-0000-4000-8000-000000000002', 'parity-manual-91002@bike.local', 'parity-manual-91002', 'Parity Segment Rebuild User', false, now()),
  (91003, '91003000-0000-4000-8000-000000000003', 'parity-manual-91003@bike.local', 'parity-manual-91003', 'Parity XC Backfill User', false, now()),
  (91004, '91004000-0000-4000-8000-000000000004', 'parity-manual-91004@bike.local', 'parity-manual-91004', 'Parity Import Reprocess User', false, now()),
  (91005, '91005000-0000-4000-8000-000000000005', 'parity-manual-91005@bike.local', 'parity-manual-91005', 'Parity Duplicate Cleanup User', false, now())
ON CONFLICT (id) DO NOTHING;
