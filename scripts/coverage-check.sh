#!/usr/bin/env bash
# Native diff-cover owns LCOV parsing and changed-line coverage calculations.
set -euo pipefail
cd "${COVERAGE_REPO_DIR:-$(dirname "$0")/..}"
: "${COVERAGE_MINIMUM:?Run through mise run coverage:check}"
# First-parent history keeps activation stable across the repository's rebase merges.
activation="${COVERAGE_BASELINE_REVISION:-$(git log --first-parent --diff-filter=A --format=%H -- scripts/coverage-check.sh | tail -1)}"
: "${activation:?Coverage activation history is required}"

comparison_ref() {
  if [[ -n "${COVERAGE_COMPARE_REF:-}" ]]; then
    printf '%s\n' "$COVERAGE_COMPARE_REF"
    return
  fi
  case "${CI_PIPELINE_EVENT:-local}" in
    pull_request)
      local target="${CI_COMMIT_TARGET_BRANCH:?PR target branch is required}"
      git check-ref-format "refs/heads/$target"
      git fetch --quiet --no-tags origin "refs/heads/$target"
      printf 'FETCH_HEAD\n'
      ;;
    push|manual)
      local previous="${CI_PREV_COMMIT_SHA:-}"
      if [[ "${CI_PREV_PIPELINE_EVENT:-}" == push &&
            "${CI_PREV_COMMIT_BRANCH:-}" == "${CI_COMMIT_BRANCH:-}" &&
            "$previous" =~ ^[0-9a-f]{40}$ ]] &&
          git merge-base --is-ancestor "$previous" HEAD; then
        printf '%s\n' "$previous"
      else
        git rev-parse HEAD^
      fi
      ;;
    local) printf 'origin/main\n' ;;
    *) printf 'Unsupported coverage event\n' >&2; return 1 ;;
  esac
}

base="$(git merge-base "$(comparison_ref)" HEAD)"
if ! git merge-base --is-ancestor "$activation" "$base"; then
  git merge-base --is-ancestor "$activation" HEAD
  base="$activation"
fi
mkdir -p .artifacts/coverage/diff
printf '%s\n' "$base" >.artifacts/coverage/diff/base-revision.txt
printf 'Changed-line coverage base: %s; minimum: %s%% per project\n' "$base" "$COVERAGE_MINIMUM"

check_suite() {
  local suite="$1" include="$2"
  shift 2
  local report=".artifacts/coverage/$suite/lcov.info"
  if [[ ! -s "$report" ]]; then
    printf 'Missing or empty %s coverage report\n' "$suite" >&2
    return 1
  fi
  if [[ "$suite" == nextjs ]] && ! awk '/^SF:/ && !/^SF:bike-ui\// { print "Next.js LCOV paths must be relative to the repository root: " $0; exit 1 }' "$report"; then
    return 1
  fi
  diff-cover "$report" --compare-branch "$base" --include "$include" \
    --exclude "$@" --include-untracked --show-uncovered --total-percent-float \
    --fail-under "$COVERAGE_MINIMUM" \
    --format "html:.artifacts/coverage/diff/$suite.html,json:.artifacts/coverage/diff/$suite.json,markdown:.artifacts/coverage/diff/$suite.md"
}

# Report both projects even when one fails; failures still block CI.
result=0
check_suite rust 'bike-rs/**/*.rs' \
  '*/bike-rs/migration/*' '*/bike-rs/*/tests/*' '*_tests.rs' || result=1
check_suite nextjs 'bike-ui/**/*.ts*' \
  '*.d.ts' '*.test.ts' '*.test.tsx' '*.spec.ts' '*.spec.tsx' \
  '*/__tests__/*' '*/bike-ui/tests/*' || result=1
exit "$result"
