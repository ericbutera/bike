#!/usr/bin/env bash
# Stop a compiler process group when the shared CI cache exceeds its budget.
set -euo pipefail
root="${BIKE_CI_CACHE_ROOT:-/cache/bike}"
limit="${BIKE_CI_CACHE_MAX_BYTES:?Run through mise}"
interval="${BIKE_CI_CACHE_POLL_SECONDS:-5}"
scanner="$(dirname "$0")/ci-cache-size.mjs"
state="$(mktemp -d)"
command_pid=''
monitor_pid=''
cleanup() {
  if [[ -n "$monitor_pid" ]]; then kill "$monitor_pid" 2>/dev/null || true; wait "$monitor_pid" 2>/dev/null || true; fi
  if [[ -n "$command_pid" ]] && kill -0 -- "-$command_pid" 2>/dev/null; then
    kill -TERM -- "-$command_pid" 2>/dev/null || true
    sleep 1
    kill -KILL -- "-$command_pid" 2>/dev/null || true
  fi
  rm -rf -- "$state"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
check_budget() {
  local allocated
  allocated="$(mise exec node -- node "$scanner" "$root")" || return 1
  if ((allocated > limit)); then
    printf 'CI cache budget exceeded: %s bytes > %s bytes at %s. Idle retention must reclaim space before compilation.\n' \
      "$allocated" "$limit" "$root" >&2
    return 1
  fi
}
check_budget || exit 73
setsid "$@" &
command_pid=$!
(
  trap - EXIT INT TERM
  while kill -0 "$command_pid" 2>/dev/null; do
    sleep "$interval"
    if ! check_budget; then
      touch "$state/exceeded"
      kill -TERM -- "-$command_pid" 2>/dev/null || true
      exit 0
    fi
  done
) &
monitor_pid=$!
result=0
wait "$command_pid" || result=$?
if [[ -f "$state/exceeded" ]] || ! check_budget; then result=73; fi
cleanup
trap - EXIT
exit "$result"
