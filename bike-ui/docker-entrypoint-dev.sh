#!/bin/sh
set -eu

CI=true pnpm install --frozen-lockfile --prod=false

exec "$@"
