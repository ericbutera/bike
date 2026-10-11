#!/usr/bin/env bash
# Share measured reports between the existing unit and database CI workflows.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${CI_PIPELINE_NUMBER:?Pipeline identity is required}"
store="/cache/bike/coverage-reports/$CI_PIPELINE_NUMBER"
case "${1:?Use save or restore}" in
  save)
    mkdir -p "$store"
    cp -a .artifacts/coverage/. "$store/"
    find /cache/bike/coverage-reports -mindepth 1 -maxdepth 1 -type d -mtime +7 -exec rm -rf -- {} +
    ;;
  restore)
    test -s "$store/bike-ui/lcov.info"
    test -s "$store/map-renderer/lcov.info"
    mkdir -p .artifacts/coverage
    cp -a "$store/." .artifacts/coverage/
    rm -rf -- "$store"
    ;;
  *) printf 'Use save or restore\n' >&2; exit 1 ;;
esac

