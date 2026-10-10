#!/usr/bin/env bash
# Compare production-shaped renderers on fixed styles, or smoke the live provider.
set -euo pipefail
cd "$(dirname "$0")/.."
repo="$PWD"
mode="${1:-fixture}"
case "$mode" in fixture|live) ;; *) printf 'Expected fixture or live mode\n' >&2; exit 1 ;; esac
temporary="$(mktemp -d "${TMPDIR:-/tmp}/bike-renderer-parity.XXXXXX")"
network="bike-renderer-parity-$RANDOM"
containers=()
cleanup() {
  local result=$?
  trap - EXIT
  for container in ${containers[@]+"${containers[@]}"}; do
    if ((result != 0)); then docker logs "$container" >&2 || result=1; fi
    docker rm --force "$container" >/dev/null || result=1
  done
  docker network rm "$network" >/dev/null || result=1
  rm -rf -- "$temporary"
  exit "$result"
}
docker network create "$network" >/dev/null
trap cleanup EXIT
mkdir -p .artifacts/map-renderer-parity
mounts=()
baseline_url=''
if [[ "$mode" == fixture ]]; then
  mkdir -p "$temporary/legacy"
  retirement="$(git log -1 --diff-filter=D --format=%H -- map-renderer/server.mjs)"
  base="${retirement:+$retirement^}"
  if [[ -z "$base" ]]; then base="$(git merge-base origin/main HEAD)"; fi
  git archive "$base" map-renderer proto | tar -x -C "$temporary/legacy"
  docker build --tag bike-map-renderer:parity-legacy \
    --build-arg NODE_IMAGE --build-arg NPM_VERSION --build-arg PLAYWRIGHT_IMAGE \
    --file "$temporary/legacy/map-renderer/Dockerfile" "$temporary/legacy"
  mounts=(--mount "type=bind,source=$repo/map-renderer/tests/styles,target=/app/styles,readonly")
  legacy="$network-legacy"
  docker run --detach --name "$legacy" --network "$network" --network-alias legacy \
    --env OTEL_TRACES_EXPORTER=none --env MAP_SERVICE_TOKEN=parity-secret \
    --env MAP_IMAGE_CACHE_DIR=/tmp/map-cache --env MAP_IMAGE_CACHE_TTL_SECONDS=2 \
    ${mounts[@]+"${mounts[@]}"} bike-map-renderer:parity-legacy >/dev/null
  containers+=("$legacy")
  baseline_url=http://legacy:3100
fi
golang="$network-golang"
docker run --detach --name "$golang" --network "$network" --network-alias golang \
  --env OTEL_TRACES_EXPORTER=none --env MAP_SERVICE_TOKEN=parity-secret \
  ${mounts[@]+"${mounts[@]}"} bike-map-renderer:review >/dev/null
containers+=("$golang")
docker run --rm --network "$network" \
  --mount "type=bind,source=$repo/map-renderer,target=/work,readonly" \
  --mount "type=bind,source=$repo/proto,target=/proto,readonly" \
  --mount "type=bind,source=$repo/.artifacts/map-renderer-parity,target=/output" \
  --env MAP_SERVICE_TOKEN=parity-secret --env "PARITY_BASELINE_URL=$baseline_url" \
  --env "PARITY_MODE=$mode" --env "PARITY_OUTPUT=/output/$mode.json" \
  "${NODE_IMAGE:?Run through mise}" node /work/tests/parity.mjs
for container in ${containers[@]+"${containers[@]}"}; do
  peak="$(docker exec "$container" cat /sys/fs/cgroup/memory.peak)"
  printf '%s peak cgroup memory bytes: %s\n' "$container" "$peak"
done
docker image inspect --format '{{.RepoTags}} image bytes: {{.Size}}' bike-map-renderer:review
if [[ "$mode" == fixture ]]; then
  docker image inspect --format '{{.RepoTags}} image bytes: {{.Size}}' bike-map-renderer:parity-legacy
fi
