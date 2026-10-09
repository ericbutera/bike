#!/usr/bin/env bash
# Small tooling fixtures; no application, database, integration, or E2E services.
set -euo pipefail
# Git hooks export repository-local variables; temporary fixture repos must not inherit them.
while IFS= read -r variable; do unset "$variable"; done < <(git rev-parse --local-env-vars)
cd "$(dirname "$0")/.."
gate="$PWD/scripts/coverage-check.sh"
fixture="$(mktemp -d "${TMPDIR:-/tmp}/bike-coverage-test.XXXXXX")"
cleanup() {
  local result=$?
  if ((result != 0)); then
    for log in "$fixture/pass.log" "$fixture/fail.log"; do
      if [[ -f "$log" ]]; then cat "$log" >&2; fi
    done
  fi
  rm -rf -- "$fixture"
  exit "$result"
}
trap cleanup EXIT
git init --quiet --initial-branch=feature "$fixture"
git -C "$fixture" config user.name 'Coverage fixture'
git -C "$fixture" config user.email 'coverage@example.com'
mkdir -p "$fixture/bike-rs/bike-core/src" "$fixture/bike-ui/lib" \
  "$fixture/.artifacts/coverage/rust" "$fixture/.artifacts/coverage/nextjs" "$fixture/scripts"
cp "$gate" "$fixture/scripts/coverage-check.sh"
printf 'legacy uncovered code\n' >"$fixture/bike-ui/lib/legacy.ts"
git -C "$fixture" add .
git -C "$fixture" commit --quiet -m 'test: existing source fixture'
export COVERAGE_REPO_DIR="$fixture"
export COVERAGE_BASELINE_REVISION
COVERAGE_BASELINE_REVISION="$(git -C "$fixture" rev-parse HEAD)"
export COVERAGE_COMPARE_REF="$COVERAGE_BASELINE_REVISION" COVERAGE_MINIMUM=80

write_report() {
  local suite="$1" source="$2" hits="$3"
  {
    printf 'TN:\nSF:%s\n' "$source"
    for line in 1 2 3 4 5; do
      if ((line <= hits)); then printf 'DA:%s,1\n' "$line"; else printf 'DA:%s,0\n' "$line"; fi
    done
    printf 'LF:5\nLH:%s\nend_of_record\n' "$hits"
  } >"$fixture/.artifacts/coverage/$suite/lcov.info"
}

for source in bike-rs/bike-core/src/new.rs bike-ui/lib/new.ts; do
  printf 'one\ntwo\nthree\nfour\nfive\n' >"$fixture/$source"
done
write_report rust bike-rs/bike-core/src/new.rs 4
write_report nextjs bike-ui/lib/new.ts 4
{
  printf 'TN:\nSF:bike-ui/lib/legacy.ts\nDA:1,0\nLF:1\nLH:0\nend_of_record\n'
} >>"$fixture/.artifacts/coverage/nextjs/lcov.info"
bash "$gate" >"$fixture/pass.log" 2>&1
COVERAGE_BASELINE_REVISION='' bash "$gate" >"$fixture/pass.log" 2>&1
test "$(cat "$fixture/.artifacts/coverage/diff/base-revision.txt")" = "$COVERAGE_BASELINE_REVISION"
jq -e '.total_num_lines == 5 and .total_percent_covered == 80' \
  "$fixture/.artifacts/coverage/diff/rust.json" >/dev/null
jq -e '.total_num_lines == 5 and .total_percent_covered == 80' \
  "$fixture/.artifacts/coverage/diff/nextjs.json" >/dev/null

write_report rust bike-rs/bike-core/src/new.rs 3
if bash "$gate" >"$fixture/fail.log" 2>&1; then
  printf 'Below-minimum Rust coverage incorrectly passed\n' >&2
  exit 1
fi
write_report rust bike-rs/bike-core/src/new.rs 4
write_report nextjs bike-ui/lib/new.ts 3
if bash "$gate" >"$fixture/fail.log" 2>&1; then
  printf 'Below-minimum Next.js coverage incorrectly passed\n' >&2
  exit 1
fi

write_report nextjs bike-ui/lib/new.ts 4
mkdir -p "$fixture/bike-rs/migration/src"
printf 'uncovered migration\n' >"$fixture/bike-rs/migration/src/new.rs"
printf 'TN:\nSF:bike-rs/migration/src/new.rs\nDA:1,0\nLF:1\nLH:0\nend_of_record\n' \
  >>"$fixture/.artifacts/coverage/rust/lcov.info"
bash "$gate" >"$fixture/pass.log" 2>&1
jq -e '.total_num_lines == 5' "$fixture/.artifacts/coverage/diff/rust.json" >/dev/null
printf 'modified legacy code\n' >"$fixture/bike-ui/lib/legacy.ts"
printf 'TN:\nSF:bike-ui/lib/legacy.ts\nDA:1,0\nLF:1\nLH:0\nend_of_record\n' \
  >>"$fixture/.artifacts/coverage/nextjs/lcov.info"
if bash "$gate" >"$fixture/fail.log" 2>&1; then
  printf 'Modified legacy code incorrectly remained exempt\n' >&2
  exit 1
fi
printf 'legacy uncovered code\n' >"$fixture/bike-ui/lib/legacy.ts"
printf '' >"$fixture/.artifacts/coverage/rust/lcov.info"
if bash "$gate" >"$fixture/fail.log" 2>&1; then
  printf 'Empty Rust coverage report incorrectly passed\n' >&2
  exit 1
fi
printf 'Coverage fixtures passed: 80%% accepted, 60%% rejected per project, legacy and migrations exempt.\n'
