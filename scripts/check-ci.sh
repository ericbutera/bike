#!/usr/bin/env bash
# Native YAML queries guard release ordering and credential restrictions.
set -euo pipefail
cd "$(dirname "$0")/.."
workflow="${BIKE_CI_CONFIG:-.woodpecker/bike.yaml}"
export YQ_MERGE_ANCHOR_TO_SPEC=true
read_yaml() { yq --yaml-fix-merge-anchor-to-spec "$1" "$workflow"; }
test "$(read_yaml '.services[] | select(.name == "docker").image')" = "$DOCKER_DIND_IMAGE"
test "$(read_yaml '.services[] | select(.name == "docker").ports | join(",")')" = 5432
test "$(read_yaml '.concurrency')" = 1
for stage in test-rust test-ui test-maps test-tooling; do
  test "$(stage="$stage" read_yaml '.steps[] | select(.name == strenv(stage)).depends_on | join(",")')" = prepare
done
test "$(read_yaml '.steps[] | select(.name == "check-coverage").depends_on | sort | join(",")')" = test-maps,test-rust,test-ui
test "$(read_yaml '.steps[] | select(.name == "build-images").depends_on | sort | join(",")')" = check-coverage,test-postgres,test-tooling
test "$(read_yaml '.steps[] | select(.name == "test-images").depends_on | join(",")')" = build-images
test "$(read_yaml '.steps[] | select(.name == "stop-engine").when[0].status | sort | join(",")')" = failure,success
test "$(read_yaml '.steps[] | select(.name == "publish-coverage").depends_on | sort | join(",")')" = stop-engine,test-images
test "$(read_yaml '.steps[] | select(.name == "deploy").depends_on | join(",")')" = publish-coverage
for stage in publish-coverage deploy; do
  test "$(stage="$stage" read_yaml '.steps[] | select(.name == strenv(stage)).when[0].event | sort | join(",")')" = manual,push
  test "$(stage="$stage" read_yaml '.steps[] | select(.name == strenv(stage)).when[0].branch')" = main
done
test "$(read_yaml '[.steps[] | select(.failure != "fail")] | length')" = 0
go_cache="$(read_yaml '.steps[] | select(.name == "test-rust").environment.BIKE_GO_CACHE')"
test "$go_cache" != null
test "$(read_yaml '.steps[] | select(.name == "deploy").environment.BIKE_GO_CACHE')" = "$go_cache"
