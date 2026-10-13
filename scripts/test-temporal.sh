#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

temporal_test_database="bike_temporal_test_$$"
trap 'docker compose exec -T postgres psql --username postgres --dbname postgres --command "DROP DATABASE \"$temporal_test_database\" WITH (FORCE)" >/dev/null' EXIT
docker compose exec -T postgres psql --username postgres --dbname postgres \
  --set ON_ERROR_STOP=1 --command "CREATE DATABASE \"$temporal_test_database\""
export BIKE_CHECKS_NETWORK="${COMPOSE_PROJECT_NAME:-bike}_default"
export BIKE_STRAVA_TEST_DATABASE_URL="postgres://postgres:postgres@postgres:5432/$temporal_test_database"
export BIKE_TEMPORAL_TEST_ADDRESS=http://temporal:7233
mise run checks:docker test:temporal:postgres
