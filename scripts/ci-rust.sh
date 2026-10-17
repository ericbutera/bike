#!/usr/bin/env bash
# Reuse successful backend verification across commits with identical inputs.
# Cargo/chef still own compilation; this cache owns completed CI results only.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${CI_PIPELINE_NUMBER:?This task requires a CI pipeline identity}"
# Keep CI artifacts small without changing LLVM coverage instrumentation.
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=1 CARGO_PROFILE_TEST_DEBUG=1
stage="${1:?Use prepare, unit, or report}"
case "$stage" in prepare|unit|report) ;; *) exit 1 ;; esac

# Include source, fixtures, vendored dependencies, contracts, tool/image pins,
# and all orchestration. UI and documentation alone cannot change Rust results.
inputs=(bike-rs contracts proto scripts .woodpecker mise.toml prek.toml
  Dockerfile.checks docker-bake.hcl .dockerignore coverage-projects.json 'compose*.yaml')
git diff --quiet HEAD -- "${inputs[@]}"
test -z "$(git ls-files --others --exclude-standard -- "${inputs[@]}")"
cache="${BIKE_RUST_CHECK_CACHE:-/cache/bike/rust-checks/v1}"
key="$({
  git ls-tree -r HEAD -- "${inputs[@]}"
  uname -sm
  cat /etc/os-release
} | sha256sum)"
key="${key%% *}"
entry="$cache/$key"
reports=.artifacts/coverage
receipt="$reports/rust-unit-inputs"
mkdir -p "$cache" "$reports"

restore_verified() {
  # Manual runs deliberately revalidate, even with a matching successful entry.
  if [[ "${CI_PIPELINE_EVENT:-}" == manual || ! -s "$entry/source" || ! -f "$entry/reports.sha256" ]]; then return 1; fi
  if ! (cd "$entry" && sha256sum --check --status reports.sha256); then
    printf 'Rust verification cache is incomplete; running checks for %s\n' "$key"
    return 1
  fi
  tar -xzf "$entry/reports.tar.gz" -C "$reports" || return 1
  touch "$entry" || return 1
  printf 'Rust verification cache HIT %s; verified by %s\n' "$key" "$(cat "$entry/source")"
  printf 'Reused formatting, Clippy, unit/integration/PostgreSQL tests, and coverage; no backend Cargo commands.\n'
}

prune_verified() {
  local expired
  while IFS= read -r -d '' expired; do
    (
      flock --exclusive --nonblock 8 || exit 0
      # Recheck age under the lock: a cache reader may just have refreshed it.
      find "$expired" -maxdepth 0 -type d -mtime +7 -exec rm -rf -- {} +
    ) 8>"$expired.lock"
  done < <(find "$cache" -mindepth 1 -maxdepth 1 -type d -name '[0-9a-f]*' -mtime +7 -print0)
}

save_verified() {
  local temporary project artifact project_list
  local -a projects=()
  project_list="$(mise exec jq -- jq -r '.[] | select(.language == "Rust") | .id' coverage-projects.json)"
  test -n "$project_list"
  while IFS= read -r project; do projects+=("$project"); done <<<"$project_list"
  for project in "${projects[@]}"; do
    for artifact in html/index.html lcov.info coverage-summary.json; do
      test -s "$reports/$project/$artifact"
    done
  done
  temporary="$(mktemp -d "$cache/.publish.XXXXXX")"
  tar -czf "$temporary/reports.tar.gz" -C "$reports" "${projects[@]}"
  (cd "$temporary" && sha256sum reports.tar.gz >reports.sha256)
  printf 'commit %s, pipeline %s\n' "$(git rev-parse HEAD)" "$CI_PIPELINE_NUMBER" >"$temporary/source"
  # Readers see either a complete previous entry or a complete new entry.
  # Serialize publishers of the same inputs, including forced manual refreshes.
  (
    flock 9
    rm -rf -- "$entry"
    mv "$temporary" "$entry"
  ) 9>"$cache/$key.lock"
  printf 'Rust verification cache SAVED %s\n' "$key"
  prune_verified
}

if [[ "$stage" == prepare ]]; then
  rm -f -- "$receipt" "$reports/rust-unit-reused" "$reports/rust-unit-passed"
  # Restore once, before parallel stages write measurements or reports.
  if (flock --shared 9; restore_verified) 9>"$cache/$key.lock"; then
    touch "$reports/rust-unit-reused"
  else
    printf 'Rust verification cache MISS %s; preparing coverage\n' "$key"
    mise install node
    mise --cd bike-rs install rust protoc aqua:taiki-e/cargo-llvm-cov
    mise --cd bike-rs exec rust protoc aqua:taiki-e/cargo-llvm-cov -- cargo llvm-cov clean --workspace
  fi
  printf '%s\n' "$key" >"$receipt"
  exit 0
fi

test "$(cat "$receipt")" = "$key"
if [[ "$stage" == unit ]]; then
  rm -f -- "$reports/rust-$stage-passed"
  if [[ -f "$reports/rust-unit-reused" ]]; then
    printf 'Rust verification cache HIT %s; %s checks restored at preparation\n' "$key" "$stage"
    printf '%s\n' "$key" >"$reports/rust-$stage-passed"
    exit 0
  fi
  printf 'Rust verification cache MISS %s; running %s checks\n' "$key" "$stage"
fi

case "$stage" in
  unit)
    mise exec rust protoc -- mise run rust:ci:unit
    printf '%s\n' "$key" >"$reports/rust-unit-passed"
    ;;
  report)
    # A failed or interrupted test stage cannot publish passing evidence.
    test "$(cat "$reports/rust-unit-passed")" = "$key"
    if [[ -f "$reports/rust-unit-reused" ]]; then exit 0; fi
    case "${CARGO_LLVM_COV_TARGET_DIR:?Shared coverage objects are required}" in
      /cache/bike/target/shared/llvm-cov-target) ;;
      *) printf 'Coverage target is outside the shared compiler cache\n' >&2; exit 1 ;;
    esac
    mise --cd bike-rs run coverage:report
    mise --cd bike-rs exec rust protoc aqua:taiki-e/cargo-llvm-cov -- cargo llvm-cov clean --profraw-only
    save_verified
    ;;
esac
