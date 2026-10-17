#!/usr/bin/env bash
# Exercise real input selection and report transfer; fake only expensive suites.
set -euo pipefail
while IFS= read -r variable; do unset "$variable"; done < <(git rev-parse --local-env-vars)
cd "$(dirname "$0")/.."
fixture="$(mktemp -d "${TMPDIR:-/tmp}/bike-ci-rust-test.XXXXXX")"
trap 'rm -rf -- "$fixture"' EXIT
mkdir -p "$fixture/repo/scripts" "$fixture/bin"
cp scripts/ci-rust.sh scripts/coverage-transfer.sh "$fixture/repo/scripts/"
cp coverage-projects.json "$fixture/repo/"
cat >"$fixture/bin/mise" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == exec ]]; then
  while [[ "$1" != -- ]]; do shift; done
  shift
  exec "$@"
fi
printf '%s\n' "$*" >>"$FIXTURE_COMMANDS"
if [[ -n "${FIXTURE_FAIL:-}" && "$*" == "$FIXTURE_FAIL" ]]; then exit 42; fi
case "$*" in
  *'cargo llvm-cov clean --workspace')
    rm -f .artifacts/coverage/unit-measured
    ;;
  'run rust:ci:unit')
    suite=unit
    touch ".artifacts/coverage/$suite-started"
    printf '%s measurement\n' "$suite" >".artifacts/coverage/$suite-measured"
    ;;
  '--cd bike-rs run coverage:report')
    test -s .artifacts/coverage/unit-measured
    while IFS= read -r project; do
      mkdir -p ".artifacts/coverage/$project/html"
      for file in html/index.html lcov.info coverage-summary.json; do
        cat .artifacts/coverage/unit-measured >".artifacts/coverage/$project/$file"
      done
    done < <(jq -r '.[] | select(.language == "Rust") | .id' coverage-projects.json)
    ;;
esac
SH
chmod +x "$fixture/bin/mise"
ln -s "$(command -v node)" "$fixture/bin/node"
ln -s "$(command -v jq)" "$fixture/bin/jq"
export PATH="$fixture/bin:/usr/bin:/bin" FIXTURE_COMMANDS="$fixture/commands"
# Rust verification must succeed without a Docker client or daemon.
if command -v docker >/dev/null; then exit 1; fi
export BIKE_RUST_CHECK_CACHE="$fixture/cache" BIKE_COVERAGE_TRANSFER_ROOT="$fixture/transfer"
export CI_PIPELINE_NUMBER=1 CI_PIPELINE_EVENT=pull_request CI_COMMIT_REF=refs/pull/1/merge
export CARGO_LLVM_COV_TARGET_DIR=/cache/bike/target/shared/llvm-cov-target
cd "$fixture/repo"
git init --quiet --initial-branch=main
git config user.name 'CI fixture'
git config user.email 'ci@example.test'
printf '.artifacts/\n' >.gitignore
mkdir -p bike-rs/api/src bike-ui/app contracts/openapi proto .woodpecker
printf 'backend\n' >bike-rs/api/src/lib.rs
printf 'ui\n' >bike-ui/app/page.tsx
printf 'pins\n' >mise.toml
printf 'contract\n' >contracts/openapi/openapi.json
printf 'protocol\n' >proto/maps.proto
printf 'runner\n' >.woodpecker/bike.yaml
git add .
git commit --quiet -m 'test: initial backend inputs'

run_stage() {
  bash scripts/ci-rust.sh "$1" >"$fixture/$1.log" 2>&1 || {
    cat "$fixture/$1.log" >&2
    return 1
  }
}
transfer() {
  for project in bike-ui map-renderer; do
    mkdir -p ".artifacts/coverage/$project"
    printf 'other coverage\n' >".artifacts/coverage/$project/lcov.info"
  done
  bash scripts/coverage-transfer.sh save
  rm -rf -- .artifacts/coverage
}
full_run() { run_stage prepare; run_stage unit; run_stage report; transfer; bash scripts/coverage-transfer.sh restore; }
assert_no_commands() {
  if [[ -s "$FIXTURE_COMMANDS" ]]; then cat "$FIXTURE_COMMANDS" >&2; exit 1; fi
}

# A cold backend only becomes reusable after tests and reporting succeed.
run_stage prepare
test -z "$(find "$BIKE_RUST_CHECK_CACHE" -name reports.sha256 -print -quit)"
if bash scripts/ci-rust.sh report >"$fixture/incomplete.log" 2>&1; then exit 1; fi
test -z "$(find "$BIKE_RUST_CHECK_CACHE" -name reports.sha256 -print -quit)"
run_stage unit
run_stage report
grep -Fxq 'run rust:ci:unit' "$FIXTURE_COMMANDS"
grep -Fxq -- '--cd bike-rs run coverage:report' "$FIXTURE_COMMANDS"
test -n "$(find "$BIKE_RUST_CHECK_CACHE" -name reports.sha256 -print -quit)"

# Same backend on a NEW PR/ref: no installations, Cargo checks, or test runners.
CI_PIPELINE_NUMBER=2 CI_COMMIT_REF=refs/pull/2/merge
printf 'new UI\n' >>bike-ui/app/page.tsx
git add bike-ui
git commit --quiet -m 'test: UI-only revision'
rm -rf -- .artifacts/coverage
: >"$FIXTURE_COMMANDS"
full_run
assert_no_commands
grep -q 'cache HIT' "$fixture/unit.log"
grep -Fxq 'unit measurement' .artifacts/coverage/api/lcov.info
while IFS= read -r project; do
  for artifact in html/index.html lcov.info coverage-summary.json; do
    grep -Fxq 'unit measurement' ".artifacts/coverage/$project/$artifact"
  done
done < <(jq -r '.[] | select(.language == "Rust") | .id' coverage-projects.json)

# Every relevant input type invalidates the result, including deletion/rename.
for file in bike-rs/api/src/lib.rs bike-rs/Cargo.lock mise.toml contracts/openapi/openapi.json proto/maps.proto .woodpecker/bike.yaml scripts/check.sh; do
  printf 'changed\n' >>"$file"
  git add .
  git commit --quiet -m 'test: backend input revision'
  : >"$FIXTURE_COMMANDS"
  full_run
  grep -Fxq 'run rust:ci:unit' "$FIXTURE_COMMANDS"
done
git mv bike-rs/api/src/lib.rs bike-rs/api/src/renamed.rs
git commit --quiet -m 'test: renamed backend source'
full_run
grep -q 'cache MISS' "$fixture/unit.log"

# Failure must never publish successful evidence for new backend inputs.
printf 'failure revision\n' >>bike-rs/api/src/renamed.rs
git add .
git commit --quiet -m 'test: failing backend revision'
run_stage prepare
export FIXTURE_FAIL='run rust:ci:unit'
if bash scripts/ci-rust.sh unit >"$fixture/failed.log" 2>&1; then exit 1; fi
test ! -f .artifacts/coverage/rust-unit-passed
if bash scripts/ci-rust.sh report >"$fixture/incomplete.log" 2>&1; then exit 1; fi
unset FIXTURE_FAIL
run_stage unit
run_stage report

# A manual run forces tests, even when matching evidence exists.
CI_PIPELINE_EVENT=manual
: >"$FIXTURE_COMMANDS"
full_run
grep -Fxq 'run rust:ci:unit' "$FIXTURE_COMMANDS"
CI_PIPELINE_EVENT=pull_request

# A restored snapshot survives result-cache eviction during verification.
run_stage prepare
key="$(cat .artifacts/coverage/rust-unit-inputs)"
rm -rf -- "${BIKE_RUST_CHECK_CACHE:?}/${key:?}"
: >"$FIXTURE_COMMANDS"
run_stage unit
run_stage report
assert_no_commands
full_run

# Corruption before preparation forces tests and reporting.
key="$(cat .artifacts/coverage/rust-unit-inputs)"
printf 'corrupt\n' >"$BIKE_RUST_CHECK_CACHE/$key/reports.tar.gz"
: >"$FIXTURE_COMMANDS"
full_run
grep -Fxq 'run rust:ci:unit' "$FIXTURE_COMMANDS"

# Uncommitted inputs cannot borrow a committed revision's passing result.
printf 'dirty\n' >>bike-rs/api/src/renamed.rs
if bash scripts/ci-rust.sh unit >"$fixture/dirty.log" 2>&1; then exit 1; fi
printf 'Rust CI reuse passed: UI-only/new PR executes zero backend commands; backend changes, failures, manual runs, corrupt caches, and dirty inputs are enforced.\n'
