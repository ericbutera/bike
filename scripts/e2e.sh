#!/usr/bin/env bash
# One outer lifecycle for a local or authenticated remote Docker engine.
set -euo pipefail
cd "$(dirname "$0")/.."

project="bike-e2e-$(date -u +%Y%m%d%H%M%S)-$$-$RANDOM"
export BIKE_E2E_PROJECT="$project"
export BIKE_JWT_SECRET="$project-session-only"
artifacts="${BIKE_E2E_ARTIFACT_ROOT:-$PWD/.artifacts/e2e}/$project"
mkdir -p "$artifacts"
runner="$project-browser"
runner_created=false
runtime_created=false
release_digests="$artifacts/release-digests.env"

# Local browser output and runtime caches share the capped development disk.
# CI uses a disposable remote engine and its run-owned named volumes.
runner_storage=(
  --mount "type=volume,source=${project}_map_image_cache,target=/data/cache"
  --mount "type=volume,source=${project}_ui_image_cache,target=/data/ui-cache"
)
if [[ -n "${BIKE_DOCKER_STORAGE_ROOT:-}" ]]; then
  export BIKE_E2E_MAP_CACHE="$artifacts/map-cache"
  export BIKE_E2E_UI_CACHE="$artifacts/ui-cache"
  mkdir -p "$BIKE_E2E_MAP_CACHE" "$BIKE_E2E_UI_CACHE" "$artifacts/tmp" "$artifacts/workspace"
  chmod 1777 "$artifacts/tmp"
  runner_storage=(
    --mount "type=bind,source=$BIKE_E2E_MAP_CACHE,target=/data/cache"
    --mount "type=bind,source=$BIKE_E2E_UI_CACHE,target=/data/ui-cache"
    --mount "type=bind,source=$artifacts/workspace,target=/workspace"
    --mount "type=bind,source=$artifacts,target=/workspace/bike-ui/.artifacts/playwright"
    --mount "type=bind,source=$artifacts/tmp,target=/tmp"
  )
fi

compose() {
  docker compose --project-name "$project" \
    --file compose.yaml --file compose.e2e.yaml "$@"
}

cleanup() {
  local result=$?
  trap - EXIT INT TERM
  set +e
  if [[ "$runtime_created" == true ]]; then
    if ! compose logs --no-color --timestamps >"$artifacts/services.log" 2>&1; then result=1; fi
  fi
  if [[ "$runner_created" == true ]]; then
    if [[ "$(docker inspect --format '{{.State.Running}}' "$runner")" == true ]]; then
      if ! docker stop --signal SIGINT --timeout 10 "$runner" >"$artifacts/runner-stop.log" 2>&1; then result=1; fi
    fi
    if ! docker logs "$runner" >"$artifacts/browser.log" 2>&1; then result=1; fi
    if [[ -z "${BIKE_DOCKER_STORAGE_ROOT:-}" ]]; then
      if ! docker cp "$runner:/workspace/bike-ui/.artifacts/playwright/." "$artifacts/"; then result=1; fi
    fi
    if ! docker rm --force "$runner" >"$artifacts/runner-cleanup.log" 2>&1; then result=1; fi
  fi
  if [[ "$runtime_created" == true ]]; then
    if ! compose down --volumes --remove-orphans >"$artifacts/cleanup.log" 2>&1; then result=1; fi
    local remaining
    if ! remaining="$(docker ps --all --quiet --filter "label=com.docker.compose.project=$project")"; then result=1; fi
    if ! docker volume ls --quiet --filter "label=com.docker.compose.project=$project" >>"$artifacts/remaining-resources.log"; then result=1; fi
    if ! docker network ls --quiet --filter "label=com.docker.compose.project=$project" >>"$artifacts/remaining-resources.log"; then result=1; fi
    remaining+="$(cat "$artifacts/remaining-resources.log")"
    if [[ -n "$remaining" ]]; then
      printf 'E2E resources survived cleanup: %s\n' "$remaining" >&2
      result=1
    fi
  fi
  if [[ -n "${BIKE_DOCKER_STORAGE_ROOT:-}" ]]; then
    rm -rf -- "$artifacts/workspace" "$artifacts/tmp" "$artifacts/map-cache" "$artifacts/ui-cache" || result=1
  fi
  if [[ "$result" == 0 && -n "${CI:-}" ]]; then
    cp "$release_digests" .artifacts/e2e-images.env || result=1
  fi
  printf 'E2E artifacts: %s\n' "$artifacts"
  exit "$result"
}

select_image() {
  local variable=$1 reference=$2
  if ! docker image inspect "$reference" >/dev/null 2>&1; then
    docker pull "$reference"
  fi
  docker image inspect "$reference" --format '{{json .}}' >>"$artifacts/images.jsonl"
  # IDs are immutable on both local engines and a job's disposable daemon.
  printf -v "$variable" '%s' "$(docker image inspect "$reference" --format '{{.Id}}')"
  export "${variable?}"
}

record_digest() {
  local name=$1 image=$2 reference digest
  reference="$(docker image inspect "$image" --format '{{index .RepoDigests 0}}')"
  digest="${reference##*@}"
  if [[ ! "$digest" =~ ^sha256:[0-9a-f]{64}$ ]]; then
    printf 'Release image has no immutable registry digest: %s\n' "$image" >&2
    exit 1
  fi
  printf '%s=%s\n' "$name" "$digest" >>"$release_digests"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
docker info --format '{{json .}}' >"$artifacts/engine.json"
printf '%s\n' "$(git rev-parse HEAD)" >"$artifacts/revision.txt"
git status --porcelain >"$artifacts/worktree-status.txt"
select_image BIKE_BACKEND_IMAGE "${BIKE_BACKEND_IMAGE:-bike-backend:review}"
select_image BIKE_UI_IMAGE "${BIKE_UI_IMAGE:-bike-ui:review}"
select_image BIKE_RENDERER_IMAGE "${BIKE_RENDERER_IMAGE:-bike-map-renderer:review}"
select_image BIKE_E2E_TOOLS_IMAGE "${BIKE_E2E_TOOLS_IMAGE:-bike-e2e-tools:review}"
select_image BIKE_POSTGRES_IMAGE "${BIKE_POSTGRES_IMAGE:?Run through mise}"
if [[ -n "${CI:-}" ]]; then
  : >.artifacts/e2e-images.env
  # Publish these identities only after product browser acceptance.
  printf 'BIKE_TESTED_REVISION=%s\n' "${CI_COMMIT_SHA:?}" >"$release_digests"
  record_digest BIKE_TESTED_BACKEND_DIGEST "$BIKE_BACKEND_IMAGE"
  record_digest BIKE_TESTED_UI_DIGEST "$BIKE_UI_IMAGE"
  record_digest BIKE_TESTED_MAP_DIGEST "$BIKE_RENDERER_IMAGE"
fi

# Mark ownership before creation so partial-startup failures also get cleanup.
runtime_created=true
compose config --quiet
compose create --no-build postgres migration bike-rs ui bike-maps

engine=()
if [[ "${DOCKER_HOST:-unix://}" == tcp://* ]]; then
  host="${DOCKER_HOST#tcp://}"
  host="${host%%:*}"
  address="$(getent ahostsv4 "$host" | awk 'NR == 1 { print $1 }')"
  [[ -n "$address" ]]
  engine+=(--add-host "$host:$address" --env DOCKER_HOST --env DOCKER_TLS_VERIFY
    --env DOCKER_CERT_PATH=/docker-certs)
else
  engine_socket="${DOCKER_HOST:-unix:///var/run/docker.sock}"
  engine+=(--mount "type=bind,source=${engine_socket#unix://},target=/var/run/docker.sock")
fi

runner_created=true
docker create --name "$runner" --init --shm-size=1g \
  --env TINI_KILL_PROCESS_GROUP=1 \
  --label "com.docker.compose.project=$project" \
  --network "${project}_default" "${engine[@]}" \
  --mount "type=volume,source=${project}_uploads,target=/data/uploads" \
  --mount "type=volume,source=${project}_map_styles,target=/data/styles" \
  "${runner_storage[@]}" \
  --env BIKE_E2E_MAP_CACHE --env BIKE_E2E_UI_CACHE \
  --env XDG_CACHE_HOME=/tmp/xdg --env NPM_CONFIG_CACHE=/tmp/npm \
  --env NPM_CONFIG_UPDATE_NOTIFIER=false --env PNPM_CONFIG_UPDATE_NOTIFIER=false \
  --env BIKE_E2E_PROJECT --env BIKE_JWT_SECRET \
  --env BIKE_BACKEND_IMAGE --env BIKE_UI_IMAGE \
  --env BIKE_RENDERER_IMAGE --env BIKE_E2E_TOOLS_IMAGE --env BIKE_POSTGRES_IMAGE \
  --env BIKE_TEMPORAL_TOOLS_IMAGE \
  --env BIKE_UI_URL=http://ui.e2e.test:3000 \
  --env BIKE_API_URL=http://api.e2e.test:3000/api \
  --env BIKE_TEST_ADMIN_TASK_FIXTURE=true --env PLAYWRIGHT_AUTH_STATE=1 \
  --env BIKE_TEST_EXPECT_AUTHENTICATED=true \
  --env HEATMAP_REQUIRE_ENABLED=1 --env HEATMAP_PREVIEW_FILTERS='sport=road_ride' \
  --env CI --env PLAYWRIGHT_VISUAL --env PLAYWRIGHT_THEMES \
  "$BIKE_E2E_TOOLS_IMAGE" "$@" >"$artifacts/runner.id"
if [[ "${DOCKER_HOST:-unix://}" == tcp://* ]]; then
  docker cp "${DOCKER_CERT_PATH:?Authenticated remote engine requires client certificates}/." "$runner:/docker-certs"
fi
# Copy current tracked source, including worktree edits, without an archive image.
tar -cf - bike-ui/package.json bike-ui/pnpm-lock.yaml bike-ui/pnpm-workspace.yaml \
  bike-ui/playwright.config.mjs bike-ui/tests/e2e bike-ui/public/map-styles \
  bike-rs/api/tests/fixtures/platform \
  compose.yaml compose.e2e.yaml docker/*.env \
  scripts/e2e-state.sh scripts/e2e-reset.sql \
  scripts/e2e-browser.sh scripts/fixture-sequences.sql | docker cp - "$runner:/workspace"
docker start --attach "$runner" &
wait "$!"
