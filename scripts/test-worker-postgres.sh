#!/usr/bin/env bash
# Own only this disposable PostgreSQL instance; never target application data.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${BIKE_POSTGRES_IMAGE:?Run through the owning mise task}"
worker_test_name="bike-worker-fixtures-${CI_PIPELINE_NUMBER:-local}-$$"
cleanup() {
  docker rm --force --volumes "$worker_test_name" >/dev/null
}
trap cleanup EXIT
for attempt in {1..30}; do
  if docker info >/dev/null 2>&1; then break; fi
  if [ "$attempt" -eq 30 ]; then printf 'Disposable Docker engine did not become ready\n' >&2; exit 1; fi
  sleep 1
done
worker_test_ports=()
if [[ -n "${BIKE_WORKER_TEST_DATABASE_HOST:-}" ]]; then
  worker_test_ports=(--publish "${BIKE_WORKER_TEST_DATABASE_PORT:-}:5432")
fi
docker run --detach --name "$worker_test_name" --tmpfs /var/lib/postgresql/data "${worker_test_ports[@]}" \
  --env POSTGRES_USER=worker_test --env POSTGRES_PASSWORD=worker_test \
  --env POSTGRES_DB=worker_test "$BIKE_POSTGRES_IMAGE" >/dev/null
for attempt in {1..30}; do
  if docker exec "$worker_test_name" pg_isready --username worker_test --dbname worker_test >/dev/null; then break; fi
  if [ "$attempt" -eq 30 ]; then docker logs "$worker_test_name"; exit 1; fi
  sleep 1
done
worker_test_host="${BIKE_WORKER_TEST_DATABASE_HOST:-}"
worker_test_port=5432
if [[ -n "$worker_test_host" ]]; then
  # CI steps reach the engine service, not its private nested bridge addresses.
  worker_test_port="$(docker inspect --format '{{(index (index .NetworkSettings.Ports "5432/tcp") 0).HostPort}}' "$worker_test_name")"
else
  worker_test_host="$(docker inspect --format '{{ .NetworkSettings.Networks.bridge.IPAddress }}' "$worker_test_name")"
fi
export BIKE_WORKER_TEST_DATABASE_URL="postgres://worker_test:worker_test@$worker_test_host:$worker_test_port/worker_test"
mise --cd bike-rs run test:workers:postgres
export TEST_DATABASE_URL="$BIKE_WORKER_TEST_DATABASE_URL"
mise --cd strava-gateway run test:postgres
