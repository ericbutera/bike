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
export CACHE_REGISTRY="$IMAGE_REGISTRY"
case "${CI_PIPELINE_EVENT:?CI event is required}" in
  pull_request) export CACHE_SCOPE="pr-${CI_COMMIT_PULL_REQUEST:?CI PR identity is required}" ;;
  *) export CACHE_SCOPE="branch-${CI_COMMIT_SOURCE_BRANCH:-${CI_COMMIT_BRANCH:?}}" ;;
esac
# Registry tags cannot contain slashes or spaces. Include a hash of the full
# branch identity to distinguish branch names that normalize identically.
if [[ "$CACHE_SCOPE" == branch-* ]]; then
  cache_hash="$(printf '%s' "$CACHE_SCOPE" | sha256sum)"
  cache_label="$(printf '%s' "$CACHE_SCOPE" | tr -c 'a-zA-Z0-9_.-' '-' | cut -c 1-80)"
  export CACHE_SCOPE="${cache_label}-${cache_hash:0:12}"
  if [[ "${CI_COMMIT_BRANCH:-}" == main ]]; then export CACHE_SCOPE=main; fi
fi
mkdir -p .artifacts
mise run images:validate
mise run images:push
