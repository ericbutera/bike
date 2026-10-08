#!/usr/bin/env bash
# Native tools for Playwright-owned fixtures; services start once per run.
set -euo pipefail
cd /workspace
fixtures=/workspace/bike-rs/api/tests/fixtures/platform
snapshots=/tmp/bike-e2e-snapshots

compose() {
  docker compose --project-name "${BIKE_E2E_PROJECT:?}" \
    --file compose.runtime.yaml --file compose.e2e.yaml "$@"
}

sql() {
  compose exec -T postgres psql -X --set ON_ERROR_STOP=1 --username postgres "$@"
}

prepare() {
  compose up --detach --wait postgres
  compose run --rm migration
  sql --dbname bike <<'SQL'
CREATE ROLE bike_application LOGIN PASSWORD 'e2e-application';
GRANT CONNECT ON DATABASE bike TO bike_application;
GRANT USAGE ON SCHEMA public TO bike_application;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO bike_application;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO bike_application;
UPDATE feature_flags SET enabled = true WHERE feature_key = 'enhanced_maps';
SQL
  files
  mkdir -p "$snapshots"
  compose up --detach --no-deps bike-rs ui bike-maps
  compose exec -T --user root ui chown nextjs:nextjs /app/.next/cache/images
}

seed() {
  printf '%s' "${1:?Fixture JSON required}" | compose exec -T \
    --env "BIKE_E2E_PROJECT=$BIKE_E2E_PROJECT" \
    --env DATABASE_URL=postgres://postgres:postgres@postgres:5432/bike \
    bike-rs /app/e2e-fixtures
  # PostgreSQL sequence metadata, rather than a hand-maintained table/ID list.
  sql --dbname bike <<'SQL'
SELECT format('SELECT setval(%L, COALESCE((SELECT max(%I) FROM %I.%I), 1), EXISTS (SELECT 1 FROM %I.%I))',
              pg_get_serial_sequence(format('%I.%I', table_schema, table_name), column_name),
              column_name, table_schema, table_name, table_schema, table_name)
FROM information_schema.columns
WHERE table_schema = 'public'
  AND pg_get_serial_sequence(format('%I.%I', table_schema, table_name), column_name) IS NOT NULL
\gexec
SQL
}

snapshot() {
  local name=${1:?}
  [[ "$name" =~ ^[a-z-]+$ ]]
  compose exec -T postgres pg_dump --username postgres --dbname bike \
    --data-only --disable-triggers --no-owner --no-privileges >"$snapshots/$name.sql"
  sha256sum "$snapshots/$name.sql" >"$snapshots/$name.sha256"
}

restore() {
  local name=${1:?} revision=${2:-0}
  [[ "$name" =~ ^[a-z-]+$ && "$revision" =~ ^[0-9]+$ ]]
  sha256sum --check "$snapshots/$name.sha256"
  # TRUNCATE and native COPY restore share one transaction. Tables, schemas,
  # roles and pooled connections retain their identity; only data is replaced.
  {
    cat scripts/e2e-reset.sql
    cat "$snapshots/$name.sql"
    printf 'UPDATE public.heatmap_user_states SET revision = revision + %s;\n' "$revision"
  } | sql --dbname bike --single-transaction
}

files() {
  # These are only this run's named volumes, mounted on the browser runner.
  find /data/uploads /data/cache /data/styles /data/ui-cache -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +
  cp -R "$fixtures/uploads/." /data/uploads/
  cp bike-ui/tests/e2e/fixtures/renderer-style.json /data/styles/route-light-v1.json
  cp bike-ui/tests/e2e/fixtures/renderer-style.json /data/styles/fiord-v1.json
  chown -R 65534:65534 /data/uploads
}

case "${1:?Choose prepare, seed, snapshot, restore, verify or files}" in
  prepare) prepare ;;
  seed) seed "${2:?}" ;;
  snapshot) snapshot "${2:?}" ;;
  restore) restore "${2:?}" "${3:-0}" ;;
  verify) sha256sum --check "$snapshots/"*.sha256 ;;
  identity)
    sql --dbname bike --tuples-only --no-align --command \
      "SELECT 'database', oid FROM pg_database WHERE datname = current_database() UNION ALL SELECT relname, oid FROM pg_class WHERE relnamespace = 'public'::regnamespace ORDER BY 1"
    ;;
  files) files ;;
  *) exit 1 ;;
esac
