#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
registry=registry.registry:5000
revision="${CI_COMMIT_SHA:?CI must supply its immutable source revision}"
export BIKE_BACKEND_IMAGE="$registry/bike-backend:$revision"
export BIKE_UI_IMAGE="$registry/bike-ui:$revision"
export BIKE_RENDERER_IMAGE="$registry/bike-maps:$revision"
export BIKE_E2E_ARTIFACT_ROOT="/cache/bike/e2e/${CI_PIPELINE_NUMBER:?CI pipeline identity is required}"
mkdir -p "$BIKE_E2E_ARTIFACT_ROOT"
find /cache/bike/e2e -mindepth 1 -maxdepth 1 -type d -mtime +7 -exec rm -rf -- {} +
# The adapter is local to this job; no per-commit browser image is published.
BUILDX_BUILDER=default mise run e2e:build
mise run e2e
