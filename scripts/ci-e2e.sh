#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
registry=registry.registry:5000
revision="${CI_COMMIT_SHA:?CI must supply its immutable source revision}"
export BIKE_API_IMAGE="$registry/bike-api:$revision"
export BIKE_WORKER_IMAGE="$registry/bike-worker:$revision"
export BIKE_UI_IMAGE="$registry/bike-ui:$revision"
export BIKE_RENDERER_IMAGE="$registry/bike-maps:$revision"
export BIKE_E2E_IMAGE="$registry/bike-e2e:$revision"
export BIKE_GATEWAY_IMAGE="$registry/bike-strava-gateway:$revision"
# The checkout PVC disappears after a pipeline; retain reports on the existing cache PVC.
export BIKE_E2E_ARTIFACT_ROOT="/cache/bike/e2e/${CI_PIPELINE_NUMBER:?CI pipeline identity is required}"
mkdir -p "$BIKE_E2E_ARTIFACT_ROOT"
find /cache/bike/e2e -mindepth 1 -maxdepth 1 -type d -mtime +7 -exec rm -rf -- {} +
for attempt in {1..60}; do
  if docker info >/dev/null 2>&1; then break; fi
  if [[ "$attempt" == 60 ]]; then
    printf 'Authenticated job-local Docker engine did not become ready\n' >&2
    exit 1
  fi
  sleep 1
done
mise run e2e
