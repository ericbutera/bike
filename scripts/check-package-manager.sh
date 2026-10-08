#!/bin/sh
set -eu

: "${PNPM_VERSION:?Run this check through mise run pins:check}"

manifest_version=$(npm --prefix bike-ui pkg get packageManager)
if [ "$manifest_version" != "pnpm@$PNPM_VERSION" ]; then
  printf '%s\n' 'bike-ui/package.json packageManager differs from mise; run mise run pins:sync.' >&2
  exit 1
fi
