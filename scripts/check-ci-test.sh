#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
fixture="$(mktemp -d)"
trap 'rm -rf -- "$fixture"' EXIT
bash scripts/check-ci.sh
export BIKE_CI_CONFIG="$fixture/workflow.yaml"
for mutation in \
  '(.steps[] | select(.name == "test-tooling").commands) -= ["mise run strava:sdk:check"]' \
  '(.steps[] | select(.name == "test-tooling").commands) -= ["mise run contracts:check"]' \
  '(.steps[] | select(.name == "build-images").depends_on) = ["test-rust"]' \
  '(.steps[] | select(.name == "deploy").when[0].event) = ["pull_request"]' \
  '(.steps[] | select(.name == "test-images").failure) = "ignore"' \
  '(.steps[] | select(.name == "stop-engine").when[0].status) = ["success"]'; do
  yq "$mutation" .woodpecker/bike.yaml > "$BIKE_CI_CONFIG"
  if bash scripts/check-ci.sh; then
    printf 'Unsafe CI mutation was accepted: %s\n' "$mutation" >&2
    exit 1
  fi
done
printf 'CI gate rejects missing prerequisites, PR deployment, ignored failures and incomplete cleanup.\n'
