-- Only reads the Rust database. No auth/user/source-file tables are exported.
\set ON_ERROR_STOP on
BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY;
SET LOCAL statement_timeout = '60s';
SELECT jsonb_build_object('kind', 'metadata', 'data', jsonb_build_object(
    'snapshot_at', transaction_timestamp(),
    'server_version', current_setting('server_version'),
    'activity_count', (SELECT count(*) FROM activities WHERE user_id = 1),
    'segment_count', (SELECT count(*) FROM segments),
    'effort_count', (SELECT count(*) FROM segment_efforts WHERE user_id = 1),
    'columns', (SELECT json_agg(x ORDER BY table_name, ordinal_position) FROM (
        SELECT table_name, column_name, ordinal_position, is_nullable,
            format_type(a.atttypid, a.atttypmod) AS sql_type
        FROM information_schema.columns c
        JOIN pg_class t ON t.relname = c.table_name
        JOIN pg_namespace n ON n.oid = t.relnamespace AND n.nspname = c.table_schema
        JOIN pg_attribute a ON a.attrelid = t.oid AND a.attname = c.column_name
        WHERE c.table_schema = 'public'
          AND c.table_name IN ('activities', 'segments', 'segment_efforts')
    ) x),
    'indexes', (SELECT json_agg(indexdef ORDER BY tablename, indexname)
        FROM pg_indexes WHERE schemaname = 'public'
          AND tablename IN ('activities', 'segments', 'segment_efforts')
          AND indexname NOT LIKE '%_pkey'),
    'live_relation_bytes', json_build_object(
        'activities', pg_total_relation_size('activities'),
        'segments', pg_total_relation_size('segments'),
        'segment_efforts', pg_total_relation_size('segment_efforts'))
));
SELECT jsonb_build_object('kind', 'activity', 'data', row_to_json(a))
FROM activities a WHERE user_id = 1 ORDER BY id;
SELECT jsonb_build_object('kind', 'segment', 'data', row_to_json(s))
FROM segments s ORDER BY id;
SELECT jsonb_build_object('kind', 'effort', 'data', row_to_json(e))
FROM segment_efforts e WHERE user_id = 1 ORDER BY id;
COMMIT;
