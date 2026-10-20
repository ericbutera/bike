#!/usr/bin/env bash
# Native tools for Playwright-owned fixtures; services start once per run.
set -euo pipefail
cd /workspace
fixtures=/workspace/bike-rs/api/tests/fixtures/platform
snapshots=/tmp/bike-e2e-snapshots

compose() {
  docker compose --project-name "${BIKE_E2E_PROJECT:?}" \
    --file compose.yaml --file compose.e2e.yaml --profile worker "$@"
}

sql() {
  compose exec -T postgres psql -X --set ON_ERROR_STOP=1 --username postgres "$@"
}

prepare() {
  compose up --detach --wait postgres temporal
  compose run --rm migration
  compose run --rm --no-deps migration /app/migration status
  compose run --rm --no-deps migration /app/recover-generated-imports --user-id 1 --limit 1
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
  compose up --detach --no-deps bike-rs ui bike-maps temporal-worker
  compose exec -T --user root ui chown nextjs:nextjs /app/.next/cache/images
}

stop_worker() {
  local log=/workspace/bike-ui/.artifacts/playwright/backend.log worker
  compose stop temporal-worker
  compose logs --no-color bike-rs temporal-worker > "$log"
  cat "$log"
  if grep -Ei '\b(WARN|ERROR)\b|"level":"(warn|error)"' "$log"; then exit 1; fi
  worker="$(compose ps --all --quiet temporal-worker)"
  [[ -n "$worker" ]]
  test "$(docker inspect --format '{{.State.ExitCode}}' "$worker")" = 0
  compose rm --force temporal-worker
  printf 'Backend API, worker, migrations and recovery helper passed; worker removed before fixtures.\n'
}

seed() {
  printf '%s' "${1:?Fixture JSON required}" | compose exec -T \
    --env "BIKE_E2E_PROJECT=$BIKE_E2E_PROJECT" \
    --env DATABASE_URL=postgres://postgres:postgres@postgres:5432/bike \
    bike-rs /app/e2e-fixtures
  sql --dbname bike < /workspace/scripts/fixture-sequences.sql
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
  chown -R 65534:65534 /data/uploads /data/cache
}

case "${1:?Choose prepare, stop-worker, seed, snapshot, restore, verify or files}" in
  prepare) prepare ;;
  stop-worker) stop_worker ;;
  seed) seed "${2:?}" ;;
  snapshot) snapshot "${2:?}" ;;
  restore) restore "${2:?}" "${3:-0}" ;;
  verify) sha256sum --check "$snapshots/"*.sha256 ;;
  identity)
    sql --dbname bike --tuples-only --no-align --command \
      "SELECT 'database', oid FROM pg_database WHERE datname = current_database() UNION ALL SELECT relname, oid FROM pg_class WHERE relnamespace = 'public'::regnamespace ORDER BY 1"
    ;;
  files) files ;;
  health) compose exec -T bike-maps /app/renderer healthcheck ;;
  *) exit 1 ;;
esac
