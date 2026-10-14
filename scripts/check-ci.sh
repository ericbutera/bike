#!/usr/bin/env bash
# Use the native YAML tool to enforce the parallel test graph and its mise pin.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${BIKE_POSTGRES_IMAGE:?Run through mise run ci:check}"
test "$(yq '.services[] | select(.name == "postgres").image' .woodpecker/bike.yaml)" = "$BIKE_POSTGRES_IMAGE"
test "$(yq '.services[] | select(.name == "docker").image' .woodpecker/bike.yaml)" = "$DOCKER_DIND_IMAGE"
test "$(yq '.services[] | select(.name == "docker").image' .woodpecker/e2e.yaml)" = "$DOCKER_DIND_IMAGE"
test "$(yq '.concurrency' .woodpecker/bike.yaml)" = 1
test "$(yq '.steps[] | select(.name == "prepare-tests").environment.CARGO_TARGET_DIR' .woodpecker/bike.yaml)" = /cache/bike/target/shared
test "$(yq '.steps[] | select(.name == "prepare-tests").environment.CARGO_LLVM_COV_TARGET_DIR' .woodpecker/bike.yaml)" = /cache/bike/target/shared/llvm-cov-target

for stage in test-rust test-worker-postgres test-strava-sdk test-runtime-tooling test-rust-cache test-ui-unit test-map-renderer; do
  test "$(stage="$stage" yq '.steps[] | select(.name == strenv(stage)).depends_on | join(",")' .woodpecker/bike.yaml)" = prepare-tests
done
test "$(yq '.steps[] | select(.name == "collect-rust-coverage").depends_on | join(",")' .woodpecker/bike.yaml)" = test-rust,test-worker-postgres,test-strava-sdk
yq --exit-status '.steps[] | select(.name == "build-images").depends_on | any_c(. == "test-worker-postgres")' .woodpecker/bike.yaml >/dev/null
yq --exit-status '.steps[] | select(.name == "build-images").depends_on | any_c(. == "retain-unit-coverage")' .woodpecker/bike.yaml >/dev/null
test "$(yq '.depends_on | join(",")' .woodpecker/e2e.yaml)" = bike
test "$(yq '.steps[] | select(.name == "test-ui-e2e").depends_on | join(",")' .woodpecker/e2e.yaml)" = prepare
