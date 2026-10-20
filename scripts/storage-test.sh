#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
fixture="$(mktemp -d)"
trap 'rm -rf -- "$fixture"' EXIT
mkdir "$fixture/bin"
export STORAGE_TEST_LOG="$fixture/commands"
export BIKE_STORAGE_METRICS_URL=http://fixture.invalid
cat > "$fixture/bin/docker" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$STORAGE_TEST_LOG"
case "$*" in
  'image prune --all --force --filter label=dev.bike.storage=disposable-image --filter until=24h')
    [[ "$STORAGE_CASE" != prune-fails ]] ;;
  'image ls --quiet --no-trunc') printf 'sha256:a\nsha256:b\nsha256:a\n' ;;
  'image inspect --format {{.Size}} sha256:a sha256:b')
    case "$STORAGE_CASE" in
      large-images) printf '34359738369\n0\n' ;;
      invalid-images) printf 'unknown\n' ;;
      *) printf '17179869184\n17179869184\n' ;;
    esac ;;
  *) printf 'Unexpected Docker operation: %s\n' "$*" >&2; exit 1 ;;
esac
MOCK
cat > "$fixture/bin/df" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
[[ "$*" == '-Pk /cache' ]]
available=67108864
case "$STORAGE_CASE" in
  low-local) available=67108863 ;;
  invalid-local) available=unknown ;;
esac
printf 'Filesystem 1024-blocks Used Available Capacity Mounted\n/dev/test 100000000 1 %s 1%% /cache\n' "$available"
MOCK
cat > "$fixture/bin/curl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
[[ "$STORAGE_CASE" != http-fails ]] || exit 22
now="$(date +%s)"
[[ "$STORAGE_CASE" != stale ]] || now=$((now - 121))
if [[ "$*" == *nibelheim_storage_path_used_bytes* ]]; then
  jq -n --argjson now "$now" --arg scenario "$STORAGE_CASE" '
    {status:"success",data:{resultType:"vector",result:[
      {metric:{persistentvolumeclaim:"bike-buildkit-cache",namespace:"woodpecker",mountpoint:"/data",node:"test"}, value:[$now,"85899345920"]},
      {metric:{persistentvolumeclaim:"woodpecker-cache",namespace:"woodpecker",mountpoint:"/data",node:"test"}, value:[$now,"85899345920"]},
      {metric:{persistentvolumeclaim:"registry-data",namespace:"registry",mountpoint:"/data",node:"test"}, value:[$now,"42949672960"]}
    ]}} |
    if $scenario == "missing" then .data.result = []
    elif $scenario == "duplicate" then .data.result += [.data.result[0]]
    elif $scenario == "large-registry" then .data.result[2].value[1] = "42949672961"
    elif $scenario == "large-builder" then .data.result[0].value[1] = "85899345921"
    elif $scenario == "large-cache" then .data.result[1].value[1] = "85899345921"
    elif $scenario == "invalid-ci" then .data.result[0].value[1] = "NaN"
    elif $scenario == "query-warning" then .warnings = ["partial response"]
    else . end'
else
  available=85899345920
  [[ "$STORAGE_CASE" != low-ci ]] || available=85899345919
  jq -n --argjson now "$now" --arg available "$available" \
    '{status:"success",data:{resultType:"vector",result:[{value:[$now,$available]}]}}'
fi
MOCK
chmod +x "$fixture/bin/"*
export PATH="$fixture/bin:$PATH"
export STORAGE_CASE=healthy
bash scripts/storage-local.sh > "$fixture/output"
grep -Fxq 'image prune --all --force --filter label=dev.bike.storage=disposable-image --filter until=24h' "$STORAGE_TEST_LOG"
: > "$STORAGE_TEST_LOG"
bash scripts/storage-local.sh --check > "$fixture/output"
if grep -q prune "$STORAGE_TEST_LOG"; then
  printf 'Read-only local check pruned images\n' >&2; exit 1
fi
bash scripts/storage-ci.sh > "$fixture/output"
for STORAGE_CASE in large-images invalid-images low-local invalid-local prune-fails; do
  if bash scripts/storage-local.sh > "$fixture/output" 2>&1; then
    printf 'Local capacity gate accepted %s\n' "$STORAGE_CASE" >&2; exit 1
  fi
done
for STORAGE_CASE in missing duplicate large-registry large-builder large-cache invalid-ci stale query-warning http-fails low-ci; do
  if bash scripts/storage-ci.sh > "$fixture/output" 2>&1; then
    printf 'CI capacity gate accepted %s\n' "$STORAGE_CASE" >&2; exit 1
  fi
done
printf 'Storage gates passed boundary, scoped-prune, read-only, over-budget, missing, stale and failed-measurement fixtures.\n'
