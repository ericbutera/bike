#!/bin/sh
set -eu

: "${GITHUB_TOKEN:?GITHUB_TOKEN is required}"
: "${PULUMI_IAC_REPO:?PULUMI_IAC_REPO is required}"

component="${1:?An explicit Bike component is required}"
export BIKE_SOURCE_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
# Source credentials belong to the trusted clone plugin, not the IaC deploy token.
if [ "$(git -C "$BIKE_SOURCE_DIR" rev-parse --is-shallow-repository)" = true ]; then
  echo 'Bike release ancestry requires the workflow clone to use partial=false and depth=0' >&2
  exit 1
fi

git clone --quiet "https://x-access-token:${GITHUB_TOKEN}@github.com/${PULUMI_IAC_REPO}" /tmp/pulumi-iac
sh /tmp/pulumi-iac/scripts/deploy-bike-image.sh "$component"
