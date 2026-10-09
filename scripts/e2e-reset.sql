-- Schema-independent reset, run only against this Compose project's database.
-- Native pg_dump data follows this in the same psql --single-transaction call.
SET LOCAL lock_timeout = '5s';
SELECT format('TRUNCATE TABLE %s RESTART IDENTITY',
              string_agg(format('%I.%I', schemaname, tablename), ', ' ORDER BY tablename))
FROM pg_tables
WHERE schemaname = 'public'
\gexec
