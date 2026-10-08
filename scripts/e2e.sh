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

compose() {
  docker compose --project-name "$project" \
    --file compose.runtime.yaml --file compose.e2e.yaml "$@"
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
    if ! docker cp "$runner:/workspace/bike-ui/.artifacts/playwright/." "$artifacts/"; then result=1; fi
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
git diff --binary HEAD >"$artifacts/worktree.patch"
select_image BIKE_API_IMAGE "${BIKE_API_IMAGE:-bike-api:review}"
select_image BIKE_UI_IMAGE "${BIKE_UI_IMAGE:-bike-ui:review}"
select_image BIKE_RENDERER_IMAGE "${BIKE_RENDERER_IMAGE:-bike-map-renderer:review}"
select_image BIKE_E2E_IMAGE "${BIKE_E2E_IMAGE:-bike-e2e:review}"
select_image BIKE_POSTGRES_IMAGE "${BIKE_POSTGRES_IMAGE:?Run through mise}"
if [[ -n "${CI:-}" ]]; then
  : >.artifacts/e2e-images.env
  select_image BIKE_GATEWAY_IMAGE "${BIKE_GATEWAY_IMAGE:?CI must select the gateway image}"
  # Native checks and builds validate these release components; browser tests
  # start neither the worker nor the provider gateway.
  select_image BIKE_WORKER_IMAGE "${BIKE_WORKER_IMAGE:?CI must select the worker image}"
  printf 'BIKE_TESTED_REVISION=%s\n' "${CI_COMMIT_SHA:?}" >"$release_digests"
  record_digest BIKE_TESTED_API_DIGEST "$BIKE_API_IMAGE"
  record_digest BIKE_TESTED_WORKER_DIGEST "$BIKE_WORKER_IMAGE"
  record_digest BIKE_TESTED_UI_DIGEST "$BIKE_UI_IMAGE"
  record_digest BIKE_TESTED_MAP_DIGEST "$BIKE_RENDERER_IMAGE"
  record_digest BIKE_TESTED_GATEWAY_DIGEST "$BIKE_GATEWAY_IMAGE"
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
  engine+=(--mount 'type=bind,source=/var/run/docker.sock,target=/var/run/docker.sock')
fi

runner_created=true
docker create --name "$runner" --init --shm-size=1g \
  --env TINI_KILL_PROCESS_GROUP=1 \
  --label "com.docker.compose.project=$project" \
  --network "${project}_default" "${engine[@]}" \
  --mount "type=volume,source=${project}_uploads,target=/data/uploads" \
  --mount "type=volume,source=${project}_map_styles,target=/data/styles" \
  --mount "type=volume,source=${project}_map_image_cache,target=/data/cache" \
  --mount "type=volume,source=${project}_ui_image_cache,target=/data/ui-cache" \
  --env BIKE_E2E_PROJECT --env BIKE_JWT_SECRET \
  --env BIKE_API_IMAGE --env BIKE_UI_IMAGE \
  --env BIKE_RENDERER_IMAGE --env BIKE_E2E_IMAGE --env BIKE_POSTGRES_IMAGE \
  --env BIKE_UI_URL=http://ui.e2e.test:3000 \
  --env BIKE_API_URL=http://api.e2e.test:3000/api \
  --env BIKE_TEST_ADMIN_TASK_FIXTURE=true --env PLAYWRIGHT_AUTH_STATE=1 \
  --env BIKE_TEST_EXPECT_AUTHENTICATED=true \
  --env HEATMAP_REQUIRE_ENABLED=1 --env HEATMAP_PREVIEW_FILTERS='sport=road_ride' \
  --env CI --env PLAYWRIGHT_VISUAL \
  "$BIKE_E2E_IMAGE" "$@" >"$artifacts/runner.id"
if [[ "${DOCKER_HOST:-unix://}" == tcp://* ]]; then
  docker cp "${DOCKER_CERT_PATH:?Authenticated remote engine requires client certificates}/." "$runner:/docker-certs"
fi
docker start --attach "$runner" &
wait "$!"
