#!/usr/bin/env bash
# Connect the owning Bake task to the persistent builder; no job-local daemon.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${CI_COMMIT_SHA:?CI must supply the source revision}"
if [[ ! "$CI_COMMIT_SHA" =~ ^[0-9a-f]{40}$ ]]; then
  printf 'Image revision must be a full Git commit SHA\n' >&2
  exit 1
fi
: "${BIKE_BUILDKIT_ADDR:?CI must select the persistent builder}"
: "${BIKE_BUILDKIT_CA:?CI must supply the builder CA}"
: "${BIKE_BUILDKIT_CERT:?CI must supply the client certificate}"
: "${BIKE_BUILDKIT_KEY:?CI must supply the client key}"

umask 077
certificates="$(mktemp -d "${TMPDIR:-/tmp}/bike-buildkit.XXXXXX")"
export BUILDX_CONFIG="$certificates/buildx"
builder="bike-ci-${CI_PIPELINE_NUMBER:?CI pipeline identity is required}"
cleanup() {
  local result=$?
  trap - EXIT
  docker-cli-plugin-docker-buildx rm "$builder" >/dev/null 2>&1 || result=1
  rm -rf -- "$certificates"
  exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
printf '%s\n' "$BIKE_BUILDKIT_CA" >"$certificates/ca.pem"
printf '%s\n' "$BIKE_BUILDKIT_CERT" >"$certificates/cert.pem"
printf '%s\n' "$BIKE_BUILDKIT_KEY" >"$certificates/key.pem"
unset BIKE_BUILDKIT_CA BIKE_BUILDKIT_CERT BIKE_BUILDKIT_KEY
docker-cli-plugin-docker-buildx create --name "$builder" --driver remote \
  --driver-opt "cacert=$certificates/ca.pem,cert=$certificates/cert.pem,key=$certificates/key.pem" \
  "$BIKE_BUILDKIT_ADDR"
docker-cli-plugin-docker-buildx inspect --builder "$builder" --bootstrap
export BUILDX_BUILDER="$builder"
export IMAGE_REGISTRY=registry.registry:5000
export IMAGE_TAG="$CI_COMMIT_SHA"
mkdir -p .artifacts
# Build cache belongs to this persistent builder; the registry holds releases.
IMAGE_REGISTRY='' mise run images:validate
mise run images:push
