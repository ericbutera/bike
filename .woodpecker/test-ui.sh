#!/bin/sh
set -e
npm install -g pnpm@9

cd ui-next
pnpm install --frozen-lockfile
pnpm run typecheck
pnpm run test
node --test ../map-renderer/cache.node.mjs
cmp public/map-styles/route-light-v1.json ../map-renderer/styles/route-light-v1.json
cmp public/map-styles/fiord-v1.json ../map-renderer/styles/fiord-v1.json
