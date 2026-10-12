#!/usr/bin/env bash
# Exercise two revisions using the actual CI coverage paths and native cleanup.
set -euo pipefail
cd "$(dirname "$0")/.."
repo="$PWD"
fixture="$(mktemp -d "${TMPDIR:-/tmp}/bike-rust-cache-test.XXXXXX")"
cleanup() {
  local result=$?
  if ((result != 0)); then
    find "$fixture" -maxdepth 1 -name '*.log' -exec cat {} + >&2
  fi
  rm -rf -- "$fixture"
  exit "$result"
}
trap cleanup EXIT
mkdir -p "$fixture/app/src" "$fixture/app/tests" "$fixture/dependency/src"
cat >"$fixture/app/Cargo.toml" <<'TOML'
[workspace]
[package]
name = "cache-app"
version = "0.1.0"
edition = "2024"
[dependencies]
cache-dependency = { path = "../dependency" }
TOML
cat >"$fixture/dependency/Cargo.toml" <<'TOML'
[workspace]
[package]
name = "cache-dependency"
version = "0.1.0"
edition = "2024"
TOML
printf 'pub fn value() -> u32 { 1 }\n' >"$fixture/dependency/src/lib.rs"
cat >"$fixture/app/src/lib.rs" <<'RUST'
pub mod removed;
pub fn current() -> u32 { cache_dependency::value() }
pub fn followup() -> u32 { 2 }
#[test]
fn unit() { assert_eq!(current(), 1); assert_eq!(removed::value(), 3); }
RUST
printf 'pub fn value() -> u32 { 3 }\n' >"$fixture/app/src/removed.rs"
printf '#[test]\nfn followup() { assert_eq!(cache_app::followup(), 2); }\n' >"$fixture/app/tests/followup.rs"
printf '#[test]\nfn removed_test() { assert_eq!(cache_app::current(), 1); }\n' >"$fixture/app/tests/removed_test.rs"
printf 'fn main() { assert_eq!(cache_app::current(), 1); }\n' >"$fixture/app/src/main.rs"
# Use the real task environment: task-local flag differences caused OpenAPI to
# rebuild a dependency graph that the integration-test task had just compiled.
cp bike-rs/mise.toml "$fixture/app/mise.toml"
printf '\n[tools]\nrust = "%s"\n' "$(mise current rust)" >>"$fixture/app/mise.toml"

# Every CI stage must select the same reusable target in both pipeline runs.
targets=()
while IFS= read -r target; do targets+=("$target"); done < <(
  awk '/CARGO_LLVM_COV_TARGET_DIR:/ { print $2 }' .woodpecker/*.yaml
)
test "${#targets[@]}" = 3
export CARGO_TARGET_DIR="$fixture/ordinary" RUSTFLAGS='-D warnings'
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=1 CARGO_PROFILE_TEST_DEBUG=1
export CARGO_LLVM_COV_DENY_WARNINGS=1 CARGO_LLVM_COV_TARGET_DIR
for pipeline in 1 2; do
  for target in "${targets[@]}"; do
    target="${target//\$\{CI_COMMIT_REF\}/refs/heads/cache-fixture}"
    target="${target//\$\{CI_PIPELINE_NUMBER\}/$pipeline}"
    target="${target/\/cache\/bike\/target/$fixture/target}"
    test "$target" = "$fixture/target/refs/heads/cache-fixture/llvm-cov-target"
    CARGO_LLVM_COV_TARGET_DIR="$target"
  done
  (
    cd "$fixture/app"
    cargo llvm-cov clean --workspace
    cargo llvm-cov --workspace --lib --bins --no-report -vv >"$fixture/unit-$pipeline.log" 2>&1
    cargo llvm-cov --workspace --test '*' --no-report >"$fixture/tests-$pipeline.log" 2>&1
    cargo llvm-cov report --lcov --output-path "$fixture/report-$pipeline.lcov" -vv >"$fixture/report-$pipeline.log" 2>&1
  )
  if ((pipeline == 1)); then
    grep -q 'Compiling cache-dependency' "$fixture/unit-$pipeline.log"
  else
    grep -q 'Fresh cache-dependency' "$fixture/unit-$pipeline.log"
  fi
  grep -q 'SF:.*/src/lib.rs' "$fixture/report-$pipeline.lcov"
  if ((pipeline == 1)); then
    grep -q 'SF:.*/src/removed.rs' "$fixture/report-$pipeline.lcov"
    # New revision removes both a module and a test executable.
    sed -e '/pub mod removed;/d' -e 's/assert_eq!(removed::value(), 3);/assert_eq!(followup(), 2);/' \
      "$fixture/app/src/lib.rs" >"$fixture/lib.rs"
    mv "$fixture/lib.rs" "$fixture/app/src/lib.rs"
    rm "$fixture/app/src/removed.rs" "$fixture/app/tests/removed_test.rs"
  else
    if grep -q 'removed' "$fixture/report-$pipeline.lcov"; then
      printf 'Removed source survived into the new coverage report\n' >&2; exit 1
    fi
    if grep -q 'removed_test-' "$fixture/report-$pipeline.log"; then
      printf 'Removed test executable was selected for the new report\n' >&2; exit 1
    fi
  fi
  (
    cd "$fixture/app"
    cargo llvm-cov clean --profraw-only
  )
  test -n "$(find "$CARGO_LLVM_COV_TARGET_DIR/debug/deps" -name 'libcache_dependency-*.rlib' -print -quit)"
done
# Ordinary tests and a subsequent binary build must reuse the same dependency.
# Unset the fixture's flags so the owning mise environment is what is tested.
(
  unset RUSTFLAGS
  mise --cd "$fixture/app" run test:non-unit >"$fixture/ordinary-test.log" 2>&1
  mise --cd "$fixture/app" exec -- cargo run -vv >"$fixture/ordinary-bin.log" 2>&1
)
grep -q 'Fresh cache-dependency' "$fixture/ordinary-bin.log"
if grep -q 'Compiling cache-dependency' "$fixture/ordinary-bin.log"; then
  printf 'Binary generation recompiled ordinary test dependencies\n' >&2; exit 1
fi
# The gate must preserve objects, not replace the native profile-only cleanup.
grep -Fq 'cargo llvm-cov clean --profraw-only' "$repo/scripts/ci-rust.sh"
printf 'Rust cache regression passed: warm dependency reused, removed sources/tests absent.\n'
