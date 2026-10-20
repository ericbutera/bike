#!/usr/bin/env bash
# PERF08 only: native fixtures and HTTP measurements on a disposable E2E database.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${BIKE_CHECKS_CONTAINER:?Enter the existing checks container first}"
export BIKE_API_IMAGE="${BIKE_API_IMAGE:-bike-api:review}"
revision="$(docker image inspect "$BIKE_API_IMAGE" --format '{{index .Config.Labels "org.opencontainers.image.revision"}}')"
if [[ ! "$revision" =~ ^[0-9a-f]{40}$ ]]; then
  printf 'Build a revision-labelled release API with mise run perf:build first\n' >&2
  exit 1
fi
export BIKE_PERF_PROJECT
BIKE_PERF_PROJECT="bike-e2e-perf-$(date +%s)-$$"
export BIKE_PERF_OUTPUT="$PWD/.artifacts/performance/$BIKE_PERF_PROJECT"
export BIKE_PERF_DATABASE_URL="postgres://postgres:postgres@$BIKE_PERF_PROJECT-postgres-1:5432/bike"
export BIKE_PERF_API_URL="http://$BIKE_PERF_PROJECT-bike-rs-1:3000"
export BIKE_JWT_SECRET="perf08-disposable-$BIKE_PERF_PROJECT"
export BIKE_E2E_MAP_CACHE="$BIKE_PERF_OUTPUT/map-cache"
mkdir -p "$BIKE_E2E_MAP_CACHE"
chmod 777 "$BIKE_E2E_MAP_CACHE"
compose() {
  docker compose -p "$BIKE_PERF_PROJECT" -f compose.runtime.yaml -f compose.e2e.yaml -f compose.perf.yaml "$@"
}
sql() {
  compose exec -T postgres psql -X --set ON_ERROR_STOP=1 --username postgres --dbname bike "$@"
}
cleanup() {
  local result=$?
  compose logs --no-color >"$BIKE_PERF_OUTPUT/services.log" 2>&1 || result=1
  if [[ "$network_connected" == true ]]; then
    docker network disconnect "${BIKE_PERF_PROJECT}_default" "$HOSTNAME" || result=1
  fi
  compose down --volumes --remove-orphans || result=1
  printf 'PERF08 artifacts: %s\n' "$BIKE_PERF_OUTPUT"
  exit "$result"
}
network_connected=false
trap cleanup EXIT
compose up --detach --wait postgres
compose run --rm migration
sql <<'SQL'
CREATE EXTENSION pg_stat_statements;
CREATE ROLE bike_application LOGIN PASSWORD 'e2e-application';
GRANT CONNECT ON DATABASE bike TO bike_application;
GRANT USAGE ON SCHEMA public TO bike_application;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO bike_application;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO bike_application;
SQL
docker network connect "${BIKE_PERF_PROJECT}_default" "$HOSTNAME"
network_connected=true
started=$SECONDS
mise --cd bike-rs run perf:seed
seed_seconds=$((SECONDS - started))
sql --tuples-only --no-align --command \
  'SELECT pg_database_size(current_database())' >"$BIKE_PERF_OUTPUT/database-bytes.txt"
sql --command ANALYZE
compose up --detach --no-deps bike-rs
ready=false
for _ in {1..60}; do
  if curl --silent --fail --output /dev/null --cookie 'refresh_token=perf08-small-owner-1' \
      "$BIKE_PERF_API_URL/api/auth/current"; then ready=true; break; fi
  sleep 1
done
[[ "$ready" == true ]]
mise --cd bike-rs run perf:measure
sql --command "EXPLAIN (ANALYZE, BUFFERS) SELECT * FROM activities WHERE user_id = 1 ORDER BY started_at DESC, id DESC LIMIT 20" >"$BIKE_PERF_OUTPUT/activity-list-plan.txt"
sql --command "EXPLAIN (ANALYZE, BUFFERS) SELECT * FROM segment_efforts WHERE segment_id = 1 ORDER BY duration_seconds, id" >"$BIKE_PERF_OUTPUT/segment-efforts-plan.txt"
docker inspect "$BIKE_PERF_PROJECT-bike-rs-1" "$BIKE_PERF_PROJECT-postgres-1" \
  --format '{{json .}}' | jq -s 'map({image: .Image, limits: {memory: .HostConfig.Memory, nano_cpus: .HostConfig.NanoCpus}})' \
  >"$BIKE_PERF_OUTPUT/runtime.json"
sql --tuples-only --no-align --command 'SHOW server_version' >"$BIKE_PERF_OUTPUT/postgres-version.txt"
runner_source_sha256="$(sha256sum scripts/perf-small.sh compose.perf.yaml bike-rs/api/tests/performance.rs bike-rs/api/tests/performance/*.rs | sha256sum | cut -d " " -f 1)"
runner_dirty=false
if [[ -n "$(git status --porcelain)" ]]; then runner_dirty=true; fi
jq -n --argjson runner_dirty "$runner_dirty" --arg runner_source_sha256 "$runner_source_sha256" --arg api_revision "$revision" --arg runner_revision "$(git rev-parse HEAD)" \
  --arg architecture "$(uname -m)" --argjson seed_seconds "$seed_seconds" \
  --slurpfile fixture "$BIKE_PERF_OUTPUT/manifest.json" \
  --slurpfile measurements "$BIKE_PERF_OUTPUT/measurements.json" \
  --slurpfile runtime "$BIKE_PERF_OUTPUT/runtime.json" \
  --rawfile database_bytes "$BIKE_PERF_OUTPUT/database-bytes.txt" \
  --rawfile postgres_version "$BIKE_PERF_OUTPUT/postgres-version.txt" \
  '{api_revision: $api_revision, runner_revision: $runner_revision, runner_dirty: $runner_dirty,
    runner_source_sha256: $runner_source_sha256, architecture: $architecture,
    seed_seconds: $seed_seconds, fixture: $fixture[0], measurements: $measurements[0],
    runtime: $runtime[0], database_bytes: ($database_bytes | rtrimstr("\n") | tonumber), postgres_version: ($postgres_version | rtrimstr("\n"))}' \
  >"$BIKE_PERF_OUTPUT/report.json"
jq '.measurements.runs[] | {scenario, run, p50_ms, p95_ms, errors, response_bytes_max, sql}' "$BIKE_PERF_OUTPUT/report.json"
