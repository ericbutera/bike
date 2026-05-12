#!/bin/sh
set -e
npm install -g pnpm@9
(cd ../kaleido/typescript/packages/kaleido && pnpm install --no-frozen-lockfile)
cd ui-next
pnpm install --frozen-lockfile
pnpm run typecheck
pnpm run test
