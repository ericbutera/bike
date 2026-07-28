#!/bin/sh
set -e
npm install -g pnpm@9

KALEIDO_DIR="${KALEIDO_DIR:-kaleido}"
if [ ! -d "$KALEIDO_DIR/typescript/packages/kaleido" ]; then
  KALEIDO_DIR="../kaleido"
fi

(cd "$KALEIDO_DIR/typescript/packages/kaleido" && pnpm install --no-frozen-lockfile)
cd ui-next
pnpm install --frozen-lockfile
pnpm run typecheck
pnpm run test
