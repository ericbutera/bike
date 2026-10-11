#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p .artifacts
scratch=$(mktemp -d "$PWD/.artifacts/strava-sdk.XXXXXX")
host_repo=${BIKE_DOCKER_HOST_WORKSPACE:-$PWD}
host_scratch="$host_repo/.artifacts/${scratch##*/}"
trap 'rm -rf "$scratch"' EXIT
mise exec jq -- jq --slurpfile overlay contracts/strava/oauth-webhooks.json \
  -f contracts/strava/prepare.jq contracts/strava/vendor-swagger.json > "$scratch/swagger.json"
mkdir -p "$scratch/vendor"
for schema in contracts/strava/vendor/*.json; do
  mise exec jq -- jq -f contracts/strava/local-refs.jq \
    "$schema" > "$scratch/vendor/${schema##*/}"
done
docker run --rm --mount "type=bind,source=$host_scratch,target=/generated" \
  "${STRAVA_SDK_GENERATOR_IMAGE:?}" generate -g rust \
  -i /generated/swagger.json -o /generated/sdk \
  --additional-properties library=reqwest,supportAsync=true,supportMiddleware=true,packageName=strava-sdk \
  --name-mappings 'type=r#type,serde_json::Value=value' \
  --model-name-mappings 'application/json=ApplicationJson' \
  --openapi-normalizer DISABLE_ALL=true \
  --skip-operation-example 2>&1 | tee "$scratch/generator.log"
if grep -Eq '\] (WARN|ERROR) ' "$scratch/generator.log"; then
  exit 1
fi
if [ "${1:-}" = check ]; then
  diff -r "$scratch/sdk/src" bike-rs/strava-sdk/src
  cmp "$scratch/sdk/Cargo.toml" bike-rs/strava-sdk/Cargo.toml
else
  mkdir -p bike-rs/strava-sdk
  rm -rf bike-rs/strava-sdk/src
  cp -R "$scratch/sdk/src" bike-rs/strava-sdk/src
  cp "$scratch/sdk/Cargo.toml" bike-rs/strava-sdk/Cargo.toml
fi
