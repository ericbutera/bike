#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
name="bike-postgres-check-$$-$RANDOM"
created=false
cleanup() {
  local result=$?
  trap - EXIT
  if [[ "$created" == true ]]; then docker rm --force --volumes "$name" >/dev/null || result=1; fi
  exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
for attempt in {1..30}; do
  if docker info >/dev/null 2>&1; then break; fi
  if [[ "$attempt" == 30 ]]; then printf 'Docker is not ready\n' >&2; exit 1; fi
  sleep 1
done
host=127.0.0.1
publish=0.0.0.0::5432
if [[ "${BIKE_CHECKS_CONTAINER:-}" == 1 ]]; then
  host=host.docker.internal
elif [[ -n "${CI:-}" ]]; then
  # Woodpecker steps and services are separate pods. Match the declared service port.
  host=docker
  publish=0.0.0.0:5432:5432
fi
created=true
docker run --detach --name "$name" --publish "$publish" \
  --env POSTGRES_DB=bike --env POSTGRES_PASSWORD=postgres \
  --env POSTGRES_INITDB_ARGS="--auth-local=scram-sha-256 --auth-host=scram-sha-256" \
  "${BIKE_POSTGRES_IMAGE:?Run through mise}" >/dev/null
for attempt in {1..30}; do
  if docker exec "$name" sh -c 'test -r "$PGDATA/postmaster.pid" && grep -q "^ready" "$PGDATA/postmaster.pid" && pg_isready -h 127.0.0.1 -U postgres' >/dev/null 2>&1; then break; fi
  if [[ "$attempt" == 30 ]]; then docker logs "$name"; exit 1; fi
  sleep 1
done
port="$(docker inspect --format '{{(index (index .NetworkSettings.Ports "5432/tcp") 0).HostPort}}' "$name")"
url="postgres://postgres:postgres@$host:$port/bike"
export BIKE_STRAVA_TEST_DATABASE_URL="$url"
export BIKE_HEATMAP_TEST_DATABASE_URL="$url"
export BIKE_INGESTION_TEST_DATABASE_URL="$url"
export BIKE_SEGMENT_TEST_DATABASE_URL="$url"
mise --cd bike-rs run test:strava:postgres
mise --cd bike-rs run test:heatmaps:postgres
mise --cd bike-rs run test:ingestion:postgres
if [[ "${BIKE_COLLECT_POSTGRES_COVERAGE:-}" == 1 ]]; then
  mise --cd bike-rs run coverage:segments:postgres
else
  mise --cd bike-rs run test:segments:postgres
fi
