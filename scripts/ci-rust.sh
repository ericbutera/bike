#!/usr/bin/env bash
# Reuse successful backend verification across commits with identical inputs.
# Cargo/chef still own compilation; this cache owns completed CI results only.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${CI_PIPELINE_NUMBER:?This task requires a CI pipeline identity}"
# Keep CI artifacts small without changing LLVM coverage instrumentation.
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=1 CARGO_PROFILE_TEST_DEBUG=1
stage="${1:?Use unit or postgres}"
case "$stage" in unit|postgres) ;; *) exit 1 ;; esac

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
  local temporary project artifact
  for project in api worker bike-core; do
    for artifact in html/index.html lcov.info coverage-summary.json; do
      test -s "$reports/$project/$artifact"
    done
  done
  temporary="$(mktemp -d "$cache/.publish.XXXXXX")"
  tar -czf "$temporary/reports.tar.gz" -C "$reports" api worker bike-core
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

if [[ "$stage" == postgres ]]; then
  # Transfer belongs here, before native coverage is merged. Restoring again
  # after native tests would overwrite their measurements with unit reports.
  bash scripts/coverage-transfer.sh restore
  test "$(cat "$receipt")" = "$key"
fi

# Hold a read lock while extracting so another successful manual run cannot
# replace the archive between checksum verification and extraction.
if (flock --shared 9; restore_verified) 9>"$cache/$key.lock"; then
  printf '%s\n' "$key" >"$receipt"
  touch "$reports/rust-unit-reused"
  exit 0
fi
printf 'Rust verification cache MISS %s; running %s checks\n' "$key" "$stage"
mise install node

case "$stage" in
  unit)
    rm -f -- "$receipt" "$reports/rust-unit-reused"
    mise --cd bike-rs install rust protoc
    bash scripts/ci-cache-guard.sh mise exec rust protoc -- mise run rust:check
    printf '%s\n' "$key" >"$receipt"
    ;;
  postgres)
    # The previous workflow must have actually validated these exact inputs.
    # Keep the native coverage target and clean only its measurements afterward.
    case "${CARGO_LLVM_COV_TARGET_DIR:?Shared coverage objects are required}" in
      /cache/bike/target/*/llvm-cov-target) ;;
      *) printf 'Coverage target is outside the reusable ref cache\n' >&2; exit 1 ;;
    esac
    if [[ -f "$reports/rust-unit-reused" ]]; then
      # Retention or a lost volume can evict a result between the two workflows.
      # Recollect units before merging native coverage in that case.
      mise --cd bike-rs install rust protoc
      bash scripts/ci-cache-guard.sh mise exec rust protoc -- mise run rust:check
    fi
    bash scripts/ci-cache-guard.sh mise run rust:workers:postgres
    mise --cd bike-rs run coverage:report
    mise --cd bike-rs exec rust protoc aqua:taiki-e/cargo-llvm-cov -- cargo llvm-cov clean --profraw-only
    save_verified
    ;;
esac
