#!/usr/bin/env bash
# Launch the existing owning task inside the Compose checks container.
set -euo pipefail
if [[ "${BIKE_CHECKS_CONTAINER:-}" == 1 ]]; then
  exec mise run "${usage_task:?Run through mise run checks:docker}"
fi
cd "$(dirname "$0")/.."
exec docker compose -p bike-checks -f compose.checks.yaml run --rm --no-deps checks mise run "${usage_task:?Run through mise run checks:docker}"
