#!/usr/bin/env bash
# Branch-selected, disposable applications. Run inside the checks container.
set -euo pipefail
cd "$(dirname "$0")/.."
storage=${BIKE_DOCKER_STORAGE_ROOT:?Enter the checks container first}
[[ ${BIKE_CHECKS_CONTAINER:-} == 1 ]] || { echo "Run inside the checks container." >&2; exit 1; }
action=${1:?Choose up, reset, down or list}
name=${2:-}
ref=${3:-}
port=${4:-3100}
scenario=${5:-baseline}

fail() { echo "$*" >&2; exit 1; }

verify_quota() {
  local options subvolume
  [[ $(findmnt -n -o FSTYPE -T "$storage") == btrfs ]] || fail "Build storage must use Btrfs."
  options=$(findmnt -n -o FS-OPTIONS -T "$storage")
  subvolume=$(sed -n 's/.*subvolid=\([0-9]*\).*/\1/p' <<<"$options")
  local limits=(/sys/fs/btrfs/*/qgroups/"0_$subvolume"/max_referenced)
  [[ ${#limits[@]} == 1 && -r ${limits[0]} ]] || fail "Cannot verify build storage quota."
  [[ $(cat "${limits[0]}") == 214748364800 ]] || fail "Build storage must have a 200 GiB quota."
}

compose() {
  # Saved values only: never inherit the developer's provider credentials or URLs.
  env -i PATH="$PATH" HOME="$HOME" DOCKER_CONFIG="${DOCKER_CONFIG:-$HOME/.docker}" \
    docker compose --project-name "$project" --env-file "$directory/environment" \
    --file "$directory/compose.runtime.yaml" --file "$directory/compose.review.yaml" "$@"
}

describe() {
  jq -r '"\(.name): \(.ref) @ \(.commit) [\(.scenario)]\nBike: \(.ui)/activities/1109\nAPI: \(.api)/api\nTemporal: \(.temporal)"' "$directory/manifest.json"
}

list() {
  local manifest
  for manifest in "$storage"/reviews/*/manifest.json; do
    [[ -f "$manifest" ]] || continue
    jq -r '[.name, .ref, .commit[0:12], .scenario, .ui] | @tsv' "$manifest"
  done
}

save_fixtures() {
  node --input-type=module - "$scenario" >"$directory/fixtures.json" <<'JS'
import { baselineData, scenarioData } from "./bike-ui/tests/e2e/helpers/seeds.mjs";
const variant = scenarioData(process.argv[2]);
process.stdout.write(JSON.stringify({
  records: [...baselineData().records, ...variant.records],
  prepare_heatmaps: variant.prepare_heatmaps,
}));
JS
  cp scripts/fixture-sequences.sql "$directory/"
  cp -R bike-rs/api/tests/fixtures/platform/uploads "$directory/uploads"
}

save_environment() {
  local commit=$1
  cp compose.runtime.yaml compose.e2e.yaml compose.review.yaml "$directory/"
  MISE_TRUSTED_CONFIG_PATHS="$source" mise --cd "$source" env --json |
    jq -r 'to_entries[] | select(.key == "BIKE_POSTGRES_IMAGE" or .key == "BIKE_TEMPORAL_TOOLS_IMAGE") | "\(.key)=\(.value)"' >"$directory/environment"
  cat >>"$directory/environment" <<EOF
REVIEW_DIRECTORY=$directory
REVIEW_API_PORT=$port
REVIEW_UI_PORT=$((port + 1))
REVIEW_TEMPORAL_PORT=$((port + 2))
BIKE_FRONTEND_URL=http://$name.localhost:$((port + 1))
BIKE_BACKEND_URL=http://$name.localhost:$port
BIKE_BROWSER_API_URL=http://$name.localhost:$port/api
BIKE_BROWSER_CDN_URL=http://$name.localhost:$port/uploads
BIKE_TEMPORAL_UI_URL=http://$name.localhost:$((port + 2))
EOF
  jq -n --arg name "$name" --arg ref "$ref" --arg commit "$commit" \
    --arg scenario "$scenario" --arg api "http://$name.localhost:$port" \
    --arg ui "http://$name.localhost:$((port + 1))" \
    --arg temporal "http://$name.localhost:$((port + 2))" \
    '{name:$name, ref:$ref, commit:$commit, scenario:$scenario, api:$api, ui:$ui, temporal:$temporal}' >"$directory/manifest.json"
}

build_images() {
  local commit=$1 image variable
  local tag="review-$name-${commit:0:12}"
  (
    cd "$source"
    IMAGE_REGISTRY="" IMAGE_TAG="$tag" MISE_TRUSTED_CONFIG_PATHS="$source" \
      mise exec -- docker-cli-plugin-docker-buildx bake --load api worker ui maps
  )
  for image in api worker ui map-renderer; do
    case "$image" in
      map-renderer) variable=BIKE_RENDERER_IMAGE ;;
      *) variable="BIKE_${image^^}_IMAGE" ;;
    esac
    # Pin content IDs so a later build cannot change an existing reset.
    printf '%s=%s\n' "$variable" "$(docker image inspect "bike-$image:$tag" --format '{{.Id}}')" >>"$directory/environment"
  done
}

start() {
  mkdir -p "$directory/cache" "$directory/ui-cache"
  compose up --detach --wait postgres temporal
  compose run --rm --no-deps migration
  compose run --rm --no-deps fixture-files
  compose run --rm --no-deps -T --env "BIKE_E2E_PROJECT=$project" \
    bike-rs /app/e2e-fixtures <"$directory/fixtures.json"
  compose exec -T postgres psql -X --set ON_ERROR_STOP=1 -U postgres -d bike <"$directory/fixture-sequences.sql"
  compose run --rm --no-deps --user root ui chown -R nextjs:nextjs /app/.next/cache
  compose up --detach --wait bike-rs ui bike-maps temporal-worker
  describe
}

up() {
  [[ -n "$ref" ]] || fail "up requires a Git branch, tag or commit."
  [[ "$port" =~ ^[0-9]{4,5}$ ]] || fail "Choose a base port between 1024 and 65532."
  port=$((10#$port))
  (( port >= 1024 && port <= 65532 )) || fail "Choose a base port between 1024 and 65532."
  [[ ! -e "$directory" && ! -e "$source" ]] || fail "$name already exists; use reset or down."
  local commit
  commit=$(git rev-parse --verify --end-of-options "$ref^{commit}")
  verify_quota
  mkdir -p "$directory"
  save_fixtures
  git worktree add --detach "$source" "$commit"
  save_environment "$commit"
  build_images "$commit"
  start
}

remove_images() {
  [[ -f "$directory/manifest.json" ]] || return 0
  local commit image tag
  commit=$(jq -r .commit "$directory/manifest.json")
  tag="review-$name-${commit:0:12}"
  for image in api worker ui map-renderer; do
    if docker image inspect "bike-$image:$tag" >/dev/null 2>&1; then
      docker image rm "bike-$image:$tag"
    fi
  done
}

down() {
  [[ -d "$directory" ]] || fail "Unknown environment: $name"
  if grep -q '^BIKE_WORKER_IMAGE=' "$directory/environment" 2>/dev/null; then
    compose down --volumes --remove-orphans
  fi
  if [[ -e "$source" ]]; then
    git worktree remove "$source"
  fi
  remove_images
  # Name validation and fixed roots keep deletion inside this named instance.
  rm -rf -- "$directory"
}

if [[ "$action" == list ]]; then list; exit; fi
[[ "$name" =~ ^[a-z][a-z0-9-]{0,31}$ ]] || fail "Use a name of 1–32 lowercase letters, digits or hyphens, starting with a letter."
directory="$storage/reviews/$name"
source="$storage/worktrees/bike-review-$name"
project="bike-e2e-review-$name"
case "$action" in
  up) up ;;
  reset)
    [[ -f "$directory/manifest.json" ]] || fail "Unknown environment: $name"
    compose down --volumes --remove-orphans
    rm -rf -- "$directory/cache" "$directory/ui-cache"
    start
    ;;
  down) down ;;
  *) fail "Choose up, reset, down or list." ;;
esac
