#!/bin/sh
set -eu

: "${GITHUB_TOKEN:?GITHUB_TOKEN is required}"
: "${PULUMI_IAC_REPO:?PULUMI_IAC_REPO is required}"

git clone --quiet "https://x-access-token:${GITHUB_TOKEN}@github.com/${PULUMI_IAC_REPO}" /tmp/pulumi-iac
sh /tmp/pulumi-iac/scripts/deploy-bike-image.sh bike
