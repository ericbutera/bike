#!/usr/bin/env bash
# Exercise real input selection and report transfer; fake only expensive suites.
set -euo pipefail
while IFS= read -r variable; do unset "$variable"; done < <(git rev-parse --local-env-vars)
cd "$(dirname "$0")/.."
fixture="$(mktemp -d "${TMPDIR:-/tmp}/bike-ci-rust-test.XXXXXX")"
trap 'rm -rf -- "$fixture"' EXIT
mkdir -p "$fixture/repo/scripts" "$fixture/bin" "$fixture/docker-bin"
cp scripts/ci-rust.sh scripts/ci-cache-guard.sh scripts/coverage-transfer.sh "$fixture/repo/scripts/"
cp scripts/test-worker-postgres.sh "$fixture/repo/scripts/"
cp scripts/ci-engine-stop.sh "$fixture/repo/scripts/"
cp scripts/ci-cache-size.mjs "$fixture/repo/scripts/"
cat >"$fixture/bin/mise" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == exec ]]; then
  tools="$*"
  while [[ "$1" != -- ]]; do shift; done
  shift
  if [[ "$tools" == *aqua:docker/cli* ]]; then PATH="$FIXTURE_DOCKER_BIN:$PATH"; fi
  exec "$@"
fi
printf '%s\n' "$*" >>"$FIXTURE_COMMANDS"
if [[ -n "${FIXTURE_FAIL:-}" && "$*" == "$FIXTURE_FAIL" ]]; then exit 42; fi
case "$*" in
  *'cargo llvm-cov clean --workspace')
    rm -f .artifacts/coverage/unit-measured .artifacts/coverage/postgres-measured
    ;;
  'run rust:ci:unit'|'run rust:workers:postgres:collect')
    if [[ "$*" == 'run rust:ci:unit' ]]; then suite=unit; other=postgres; else suite=postgres; other=unit; fi
    touch ".artifacts/coverage/$suite-started"
    if [[ "${FIXTURE_REQUIRE_PARALLEL:-false}" == true ]]; then
      for attempt in {1..100}; do
        if [[ -f ".artifacts/coverage/$other-started" ]]; then break; fi
        if [[ "$attempt" == 100 ]]; then exit 43; fi
        sleep 0.05
      done
    fi
    printf '%s measurement\n' "$suite" >".artifacts/coverage/$suite-measured"
    ;;
  '--cd bike-rs run coverage:report')
    test -s .artifacts/coverage/unit-measured
    test -s .artifacts/coverage/postgres-measured
    for project in api worker bike-core strava-temporal; do
      mkdir -p ".artifacts/coverage/$project/html"
      for file in html/index.html lcov.info coverage-summary.json; do
        cat .artifacts/coverage/unit-measured .artifacts/coverage/postgres-measured >".artifacts/coverage/$project/$file"
      done
    done
    ;;
esac
SH
chmod +x "$fixture/bin/mise"
cat >"$fixture/docker-bin/docker" <<'SH'
#!/usr/bin/env bash
[[ "${FIXTURE_ALLOW_DOCKER_INFO:-true}" == true && "${1:-}" == info ]]
SH
chmod +x "$fixture/docker-bin/docker"
ln -s "$(command -v node)" "$fixture/bin/node"
export PATH="$fixture/bin:/usr/bin:/bin" FIXTURE_COMMANDS="$fixture/commands" FIXTURE_DOCKER_BIN="$fixture/docker-bin"
# Docker is unavailable until the owning mise execution selects its tool path.
if command -v docker >/dev/null; then exit 1; fi
export BIKE_RUST_CHECK_CACHE="$fixture/cache" BIKE_COVERAGE_TRANSFER_ROOT="$fixture/transfer"
export BIKE_CI_CACHE_ROOT="$fixture/cache" BIKE_CI_CACHE_MAX_BYTES=10485760
export CI_PIPELINE_NUMBER=1 CI_PIPELINE_EVENT=pull_request CI_COMMIT_REF=refs/pull/1/merge
export CARGO_LLVM_COV_TARGET_DIR=/cache/bike/target/refs/pull/1/merge/llvm-cov-target
export BIKE_E2E_ENGINE_STATE_DIR=.artifacts/sdk-engine
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
mkdir -p "$BIKE_E2E_ENGINE_STATE_DIR"
printf '0\n' >"$BIKE_E2E_ENGINE_STATE_DIR/stopped"

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
parallel_suites() {
  rm -f .artifacts/coverage/unit-started .artifacts/coverage/postgres-started
  export FIXTURE_REQUIRE_PARALLEL=true
  run_stage unit &
  local unit_pid=$!
  run_stage postgres &
  local postgres_pid=$!
  run_stage sdk &
  local sdk_pid=$!
  wait "$unit_pid"
  wait "$postgres_pid"
  wait "$sdk_pid"
  unset FIXTURE_REQUIRE_PARALLEL
}
full_run() { run_stage prepare; parallel_suites; run_stage report; transfer; bash scripts/coverage-transfer.sh restore; }
assert_no_commands() {
  if [[ -s "$FIXTURE_COMMANDS" ]]; then cat "$FIXTURE_COMMANDS" >&2; exit 1; fi
}

# A cold backend only becomes reusable after BOTH suites and reporting succeed.
run_stage prepare
run_stage unit
test -z "$(find "$BIKE_RUST_CHECK_CACHE" -name reports.sha256 -print -quit)"
if bash scripts/ci-rust.sh report >"$fixture/incomplete.log" 2>&1; then exit 1; fi
run_stage postgres
test -z "$(find "$BIKE_RUST_CHECK_CACHE" -name reports.sha256 -print -quit)"
if bash scripts/ci-rust.sh report >"$fixture/incomplete.log" 2>&1; then exit 1; fi
run_stage sdk
run_stage report
grep -Fxq 'run rust:ci:unit' "$FIXTURE_COMMANDS"
grep -Fxq 'run rust:workers:postgres:collect' "$FIXTURE_COMMANDS"
grep -Fxq 'run strava:sdk:check' "$FIXTURE_COMMANDS"
grep -Fxq -- '--cd bike-rs run coverage:report' "$FIXTURE_COMMANDS"
test -n "$(find "$BIKE_RUST_CHECK_CACHE" -name reports.sha256 -print -quit)"

# Same backend on a NEW PR/ref: no installations, Cargo checks, or test runners.
CI_PIPELINE_NUMBER=2 CI_COMMIT_REF=refs/pull/2/merge
printf 'new UI\n' >>bike-ui/app/page.tsx
git add bike-ui
git commit --quiet -m 'test: UI-only revision'
: >"$FIXTURE_COMMANDS"
full_run
assert_no_commands
grep -q 'cache HIT' "$fixture/unit.log"
grep -Fxq 'unit measurement' .artifacts/coverage/api/lcov.info
grep -Fxq 'postgres measurement' .artifacts/coverage/api/lcov.info

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
unset FIXTURE_FAIL
run_stage unit
export FIXTURE_FAIL='run rust:workers:postgres:collect'
if bash scripts/ci-rust.sh postgres >"$fixture/failed.log" 2>&1; then exit 1; fi
if bash scripts/ci-rust.sh report >"$fixture/incomplete.log" 2>&1; then exit 1; fi
unset FIXTURE_FAIL
run_stage postgres
export FIXTURE_FAIL='run strava:sdk:check'
if bash scripts/ci-rust.sh sdk >"$fixture/failed.log" 2>&1; then exit 1; fi
if bash scripts/ci-rust.sh report >"$fixture/incomplete.log" 2>&1; then exit 1; fi
unset FIXTURE_FAIL
printf '1\n' >"$BIKE_E2E_ENGINE_STATE_DIR/stopped"
if bash scripts/ci-rust.sh sdk >"$fixture/failed-engine.log" 2>&1; then exit 1; fi
test ! -f .artifacts/coverage/rust-sdk-passed
printf '0\n' >"$BIKE_E2E_ENGINE_STATE_DIR/stopped"
run_stage sdk
run_stage report

# A manual run forces both suites, even when matching evidence exists.
CI_PIPELINE_EVENT=manual
: >"$FIXTURE_COMMANDS"
full_run
grep -Fxq 'run rust:ci:unit' "$FIXTURE_COMMANDS"
grep -Fxq 'run rust:workers:postgres:collect' "$FIXTURE_COMMANDS"
CI_PIPELINE_EVENT=pull_request

# A restored snapshot survives result-cache eviction during parallel checks.
run_stage prepare
key="$(cat .artifacts/coverage/rust-unit-inputs)"
rm -rf -- "${BIKE_RUST_CHECK_CACHE:?}/${key:?}"
: >"$FIXTURE_COMMANDS"
parallel_suites
run_stage report
assert_no_commands
full_run

# Corruption before preparation forces both suites and combined reporting.
key="$(cat .artifacts/coverage/rust-unit-inputs)"
printf 'corrupt\n' >"$BIKE_RUST_CHECK_CACHE/$key/reports.tar.gz"
: >"$FIXTURE_COMMANDS"
full_run
grep -Fxq 'run rust:ci:unit' "$FIXTURE_COMMANDS"
grep -Fxq 'run rust:workers:postgres:collect' "$FIXTURE_COMMANDS"

# Uncommitted inputs cannot borrow a committed revision's passing result.
printf 'dirty\n' >>bike-rs/api/src/renamed.rs
if bash scripts/ci-rust.sh unit >"$fixture/dirty.log" 2>&1; then exit 1; fi
printf 'Rust CI reuse passed: UI-only/new PR executes zero backend commands; backend changes, failures, manual runs, corrupt caches, and dirty inputs are enforced.\n'

# The real guard refuses over-budget builds and terminates a growing process.
mkdir -p "$fixture/guard"
export BIKE_CI_CACHE_ROOT="$fixture/guard" BIKE_CI_CACHE_MAX_BYTES=32768 BIKE_CI_CACHE_POLL_SECONDS=0.1
dd if=/dev/zero of="$fixture/guard/payload" bs=1024 count=64 status=none
if bash scripts/ci-cache-guard.sh touch "$fixture/should-not-run" >"$fixture/guard.log" 2>&1; then exit 1; fi
test ! -e "$fixture/should-not-run"
rm "$fixture/guard/payload"
if bash scripts/ci-cache-guard.sh bash -c 'dd if=/dev/zero of="$BIKE_CI_CACHE_ROOT/payload" bs=1024 count=64 status=none; sleep 30' >"$fixture/guard.log" 2>&1; then exit 1; fi
grep -q 'CI cache budget exceeded' "$fixture/guard.log"
printf 'Disk guard passed: over-budget compilation refused and growing process group stopped.\n'

# Real compilers replace temporary files during measurement; that is not an
# over-budget condition. Exercise the same create/rename/unlink race on disk.
export BIKE_CI_CACHE_MAX_BYTES=10485760 BIKE_CI_CACHE_POLL_SECONDS=0.01
bash scripts/ci-cache-guard.sh bash -c '
  for ((i=0; i<400; i++)); do
    mkdir "$BIKE_CI_CACHE_ROOT/rmeta$i"
    printf "temporary object\n" >"$BIKE_CI_CACHE_ROOT/rmeta$i/object.tmp"
    mv "$BIKE_CI_CACHE_ROOT/rmeta$i/object.tmp" "$BIKE_CI_CACHE_ROOT/rmeta$i/object.o"
    rm -r "$BIKE_CI_CACHE_ROOT/rmeta$i"
  done
'
printf 'Disk measurement passed while compiler temporary files were replaced.\n'
ln "$fixture/guard/payload" "$fixture/guard/hardlink"
expected="$(du -skx "$fixture/guard")"
expected="${expected%%[[:space:]]*}"
test "$(node scripts/ci-cache-size.mjs "$fixture/guard")" -eq "$((expected * 1024))"
if node scripts/ci-cache-size.mjs "$fixture/missing" >"$fixture/missing.log" 2>&1; then exit 1; fi
printf 'Disk accounting passed: hard links count once and a missing root fails.\n'

# Native CI uses only its supplied service; it cannot create/remove Docker data.
cat >"$fixture/bin/docker" <<'SH'
#!/usr/bin/env bash
exit 44
SH
cat >"$fixture/bin/timeout" <<'SH'
#!/usr/bin/env bash
[[ "${FIXTURE_PG_READY:-true}" == true ]]
SH
cat >"$fixture/bin/sleep" <<'SH'
#!/usr/bin/env bash
exit 0
SH
chmod +x "$fixture/bin/docker" "$fixture/bin/timeout" "$fixture/bin/sleep"
export BIKE_POSTGRES_IMAGE=synthetic-postgres BIKE_WORKER_TEST_DATABASE_HOST=postgres BIKE_WORKER_TEST_DATABASE_PORT=5432
export BIKE_WORKER_TEST_DATABASE_URL=postgres://worker_test:worker_test@postgres:5432/worker_test
: >"$FIXTURE_COMMANDS"
bash scripts/test-worker-postgres.sh
grep -Fxq -- '--cd bike-rs run test:workers:postgres' "$FIXTURE_COMMANDS"
: >"$FIXTURE_COMMANDS"
export FIXTURE_PG_READY=false
if bash scripts/test-worker-postgres.sh >"$fixture/not-ready.log" 2>&1; then exit 1; fi
assert_no_commands
printf 'Native database lifecycle passed: supplied service used without Docker, and readiness failure blocks tests.\n'
