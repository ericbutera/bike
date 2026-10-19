#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

view() {
  jq --raw-input --raw-output --argjson activities "$1" --argjson min_ms "$2" \
    --from-file scripts/db-queries.jq < scripts/fixtures/db-queries.jsonl
}

output="$(view false 0)"
[[ "$output" == *'12.346 ms returned=20 affected=0'* ]]
[[ "$output" == *'GET /api/activities activity.list trace=list-trace'* ]]
[[ "$(printf '%s\n' "$output" | grep -c "SELECT id FROM activities WHERE user_id = \$1")" == 2 ]]
[[ "$output" == *'[WARN] 350 ms returned=80 affected=0'* ]]
[[ "$output" == *'activity.get activity_id=42 trace=detail-trace'* ]]
[[ "$output" == *$'SELECT *\nFROM segment_efforts WHERE activity_id = $1'* ]]
[[ "$output" == *'startup   trace=-'* ]]
[[ "$output" == *'SELECT 1'* ]]
[[ "$output" == *'trace=segments-trace'* ]]
[[ "$output" != *'unrelated application event'* ]]
[[ "$output" != *'Finished dev profile'* ]]

output="$(view true 0)"
[[ "$output" == *'trace=list-trace'* ]]
[[ "$output" == *'trace=detail-trace'* ]]
[[ "$output" == *'trace=auth-trace'* ]]
[[ "$output" != *'trace=segments-trace'* ]]
[[ "$output" != *'SELECT 1'* ]]

output="$(view true 350)"
[[ "$output" == *'trace=detail-trace'* ]]
[[ "$output" != *'trace=list-trace'* ]]
[[ -z "$(view true 351)" ]]
printf 'SQL log viewer fixtures passed\n'
