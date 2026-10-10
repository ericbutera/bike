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
jq -e 'type == "array" and length > 0' coverage-projects.json >/dev/null
printf '%s\n' "$base" >.artifacts/coverage/diff/base-revision.txt
printf 'Changed-line coverage base: %s; minimum: %s%% per project\n' "$base" "$COVERAGE_MINIMUM"

check_project() {
  local project="$1" id source include report
  id="$(jq -r .id <<<"$project")"
  source="$(jq -r .source <<<"$project")"
  include="$(jq -r .include <<<"$project")"
  report=".artifacts/coverage/$id/lcov.info"
  local -a exclude=()
  while IFS= read -r pattern; do exclude+=("$pattern"); done < <(jq -r '.exclude[]' <<<"$project")
  if [[ ! -s "$report" ]]; then
    printf 'Missing or empty %s coverage report\n' "$id" >&2
    return 1
  fi
  if ! awk -v prefix="SF:$source/" '
    /^SF:/ { found = 1; if (index($0, prefix) != 1 || $0 ~ /\/\.\.\//) {
      print "LCOV source path is outside its project: " $0; invalid = 1
    }}
    /^DA:/ { measured = 1 }
    END { exit invalid || !found || !measured }
  ' "$report"; then
    return 1
  fi
  printf '\nCoverage project: %s\n' "$id"
  diff-cover "$report" --compare-branch "$base" --include "$include" \
    --exclude "${exclude[@]}" '*/migration/*' '*/migrations/*' \
    --include-untracked --show-uncovered --total-percent-float \
    --fail-under "$COVERAGE_MINIMUM" \
    --format "html:.artifacts/coverage/diff/$id.html,json:.artifacts/coverage/diff/$id.json,markdown:.artifacts/coverage/diff/$id.md"
}

# Report every project even when one fails; failures still block CI.
result=0
while IFS= read -r project; do
  check_project "$project" || result=1
done < <(jq -c '.[]' coverage-projects.json)
exit "$result"
