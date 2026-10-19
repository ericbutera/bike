#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
docker compose logs --follow --tail "${usage_tail:-0}" --no-log-prefix --no-color bike-rs |
  jq --unbuffered --raw-input --raw-output \
    --argjson activities "${usage_activities:-false}" \
    --argjson min_ms "${usage_min_ms:-0}" \
    --from-file scripts/db-queries.jq
