#!/bin/sh
set -e
npm install -g pnpm@9

cd ui-next
pnpm install --frozen-lockfile
pnpm run typecheck
pnpm run test
