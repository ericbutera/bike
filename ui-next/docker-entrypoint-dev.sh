#!/bin/sh
set -eu

pnpm install --no-frozen-lockfile --prod=false --config.confirmModulesPurge=false

exec "$@"
