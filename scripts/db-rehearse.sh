#!/usr/bin/env bash
set -euo pipefail

dump=${1:?Usage: mise run db:rehearse /absolute/path/to/bike.dump}
: "${BIKE_POSTGRES_IMAGE:?Run this script through mise run db:rehearse}"
: "${BIKE_POSTGRES_UPGRADE_IMAGE:?Run this script through mise run db:rehearse}"
test -s "$dump"

rehearsal_dir=$(mktemp -d)
rehearsal_name="bike-pg-rehearsal-$(basename "$rehearsal_dir" | tr '[:upper:]' '[:lower:]')"

cleanup() {
  docker rm -f "$rehearsal_name-17" "$rehearsal_name-18" >/dev/null 2>&1 || true
  docker volume rm "$rehearsal_name-17" "$rehearsal_name-18" >/dev/null 2>&1 || true
  rm -rf "$rehearsal_dir"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

wait_for_database() {
  local attempt=0
  until docker exec "$1" pg_isready -U postgres -d bike >/dev/null 2>&1; do
    attempt=$((attempt + 1))
    test "$attempt" -lt 60
    sleep 1
  done
}

table_counts() {
  docker exec -i "$1" psql -XqAt -v ON_ERROR_STOP=1 -U postgres -d bike <<'SQL'
SELECT format('SELECT %L, count(*) FROM %I.%I;', tablename, schemaname, tablename) FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename
\gexec
SQL
}

for major in 17 18; do
  image="$BIKE_POSTGRES_IMAGE"
  volume_path=/var/lib/postgresql/data
  if [ "$major" = 18 ]; then
    image="$BIKE_POSTGRES_UPGRADE_IMAGE"
    volume_path=/var/lib/postgresql
  fi
  container="$rehearsal_name-$major"
  docker volume create "$container" >/dev/null
  docker run --detach --name "$container" --publish 127.0.0.1::5432 \
    --mount "type=volume,source=$container,target=$volume_path" \
    --env POSTGRES_PASSWORD=rehearsal-only --env POSTGRES_DB=bike "$image" >/dev/null
  wait_for_database "$container"
  docker exec -i "$container" pg_restore --exit-on-error --no-owner --no-acl -U postgres -d bike < "$dump"
  docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -d bike \
    -c 'SHOW server_version; SHOW data_directory; SELECT extname, extversion FROM pg_extension ORDER BY extname;'
  table_counts "$container" > "$rehearsal_dir/$major.counts"
  port=$(docker port "$container" 5432/tcp | cut -d: -f2)
  BIKE_HEATMAP_TEST_DATABASE_URL="postgres://postgres:rehearsal-only@127.0.0.1:$port/bike" \
    mise --cd bike-rs run test:heatmaps:postgres
  docker exec "$container" createdb -U postgres gateway_test
  TEST_DATABASE_URL="postgres://postgres:rehearsal-only@127.0.0.1:$port/gateway_test" \
    mise --cd strava-gateway run test:postgres
done

cmp "$rehearsal_dir/17.counts" "$rehearsal_dir/18.counts"
docker rm -f "$rehearsal_name-18" >/dev/null
docker restart "$rehearsal_name-17" >/dev/null
wait_for_database "$rehearsal_name-17"
table_counts "$rehearsal_name-17" > "$rehearsal_dir/recovery.counts"
cmp "$rehearsal_dir/17.counts" "$rehearsal_dir/recovery.counts"
echo 'PostgreSQL 17/18 restore, all public table counts, and retained-17 recovery passed.'
