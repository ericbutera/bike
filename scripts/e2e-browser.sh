#!/usr/bin/env bash
set -euo pipefail
cd /workspace/bike-ui
pnpm install --frozen-lockfile
exec xvfb-run --auto-servernum pnpm exec playwright test "$@"
