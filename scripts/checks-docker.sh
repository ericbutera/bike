#!/usr/bin/env bash
# Run owning tasks with Linux tool/build caches and disposable containers.
set -euo pipefail
cd "$(dirname "$0")/.."
repo="$PWD"
git_common="$(git rev-parse --path-format=absolute --git-common-dir)"
docker build --file Dockerfile.checks --tag bike-checks:local \
  --build-arg MISE_IMAGE --build-arg RUST_IMAGE --build-arg DOCKER_CLI_IMAGE .
docker run --rm --init \
  --mount "type=bind,source=$repo,target=/workspace" \
  --mount "type=bind,source=$git_common,target=$git_common" \
  --mount type=bind,source=/var/run/docker.sock,target=/var/run/docker.sock \
  --mount type=volume,source=bike-checks-cache,target=/cache \
  --mount type=volume,source=bike-checks-ui-deps,target=/workspace/bike-ui/node_modules \
  --mount type=volume,source=bike-checks-renderer-deps,target=/workspace/map-renderer/node_modules \
  --env MISE_DATA_DIR=/cache/mise --env MISE_CACHE_DIR=/cache/mise-cache \
  --env MISE_TRUSTED_CONFIG_PATHS=/workspace --env MISE_YES=1 \
  --env MISE_TASK_RUN_AUTO_INSTALL=true \
  --env CARGO_HOME=/cache/cargo --env RUSTUP_HOME=/cache/rustup \
  --env CARGO_TARGET_DIR=/cache/target \
  --env CARGO_LLVM_COV_TARGET_DIR=/cache/target/llvm-cov-target \
  --env UV_CACHE_DIR=/cache/uv --env PREK_HOME=/cache/prek \
  --env BIKE_NODE_PACKAGE_CACHE=/cache/node-packages --env BIKE_GO_CACHE=/cache/go \
  --env GOCACHE=/cache/go/go-build --env GOMODCACHE=/cache/go/go-mod \
  --env GIT_CONFIG_COUNT=1 --env GIT_CONFIG_KEY_0=safe.directory --env GIT_CONFIG_VALUE_0=/workspace \
  --env COVERAGE_COMPARE_REF bike-checks:local "${usage_task:?Run through mise run checks:docker}"
