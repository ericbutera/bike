#!/usr/bin/env bash
# Keep native LCOV portable across developer worktrees and CI checkouts.
set -euo pipefail
repo="${COVERAGE_REPO_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
report="${1:?LCOV report path is required}"
awk -v prefix="SF:$repo/" -v physical="SF:$(cd "$repo" && pwd -P)/" '
  index($0, prefix) == 1 { $0 = "SF:" substr($0, length(prefix) + 1) }
  index($0, physical) == 1 { $0 = "SF:" substr($0, length(physical) + 1) }
  { print }
' "$report" >"$report.next"
mv "$report.next" "$report"
