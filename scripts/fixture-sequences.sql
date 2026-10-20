-- PostgreSQL sequence metadata keeps explicit fixture IDs safe for later inserts.
SELECT format('SELECT setval(%L, COALESCE((SELECT max(%I) FROM %I.%I), 1), EXISTS (SELECT 1 FROM %I.%I))',
              pg_get_serial_sequence(format('%I.%I', table_schema, table_name), column_name),
              column_name, table_schema, table_name, table_schema, table_name)
FROM information_schema.columns
WHERE table_schema = 'public'
  AND pg_get_serial_sequence(format('%I.%I', table_schema, table_name), column_name) IS NOT NULL
\gexec
