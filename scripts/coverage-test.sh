#!/usr/bin/env bash
# Isolated coverage-policy, native Go adapter, and static-site unit fixtures.
set -euo pipefail
while IFS= read -r variable; do unset "$variable"; done < <(git rev-parse --local-env-vars)
cd "$(dirname "$0")/.."
repo="$PWD"
gate="$repo/scripts/coverage-check.sh"
fixture="$(mktemp -d "${TMPDIR:-/tmp}/bike-coverage-test.XXXXXX")"
cleanup() {
  local result=$?
  if ((result != 0)); then
    for log in "$fixture/pass.log" "$fixture/fail.log" "$fixture/go.log"; do
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
mkdir -p "$fixture/scripts" "$fixture/bike-ui/lib"
cp scripts/coverage-check.sh scripts/coverage-normalize.sh "$fixture/scripts/"
cp coverage-projects.json "$fixture/"
printf 'legacy uncovered code\n' >"$fixture/bike-ui/lib/legacy.ts"
git -C "$fixture" add .
git -C "$fixture" commit --quiet -m 'test: existing source fixture'
export COVERAGE_REPO_DIR="$fixture"
export COVERAGE_BASELINE_REVISION
COVERAGE_BASELINE_REVISION="$(git -C "$fixture" rev-parse HEAD)"
export COVERAGE_COMPARE_REF="$COVERAGE_BASELINE_REVISION" COVERAGE_MINIMUM=80

source_file() {
  jq -r --arg id "$1" '.[] | select(.id == $id) | .source + (if .language == "Rust" then "/src/new.rs" elif .language == "Next.js" then "/lib/new.ts" elif .language == "Go" then "/new.go" else "/new.mjs" end)' "$fixture/coverage-projects.json"
}
write_report() {
  local id="$1" hits="$2"
  {
    printf 'TN:\nSF:%s\n' "$(source_file "$id")"
    for line in 1 2 3 4 5; do
      if ((line <= hits)); then printf 'DA:%s,1\n' "$line"; else printf 'DA:%s,0\n' "$line"; fi
    done
    printf 'LF:5\nLH:%s\nend_of_record\n' "$hits"
  } >"$fixture/.artifacts/coverage/$id/lcov.info"
}
ids=()
while IFS= read -r id; do
  ids+=("$id")
  file="$(source_file "$id")"
  mkdir -p "$fixture/$(dirname "$file")" "$fixture/.artifacts/coverage/$id/html"
  printf 'one\ntwo\nthree\nfour\nfive\n' >"$fixture/$file"
  write_report "$id" 4
  printf '<html><body>Native report fixture</body></html>\n' >"$fixture/.artifacts/coverage/$id/html/index.html"
done < <(jq -r '.[].id' "$fixture/coverage-projects.json")
printf 'TN:\nSF:bike-ui/lib/legacy.ts\nDA:1,0\nLF:1\nLH:0\nend_of_record\n' >>"$fixture/.artifacts/coverage/bike-ui/lcov.info"
bash "$gate" >"$fixture/pass.log" 2>&1
COVERAGE_BASELINE_REVISION='' bash "$gate" >"$fixture/pass.log" 2>&1
test "$(cat "$fixture/.artifacts/coverage/diff/base-revision.txt")" = "$COVERAGE_BASELINE_REVISION"

for id in "${ids[@]}"; do
  jq -e '.total_num_lines == 5 and .total_percent_covered == 80' "$fixture/.artifacts/coverage/diff/$id.json" >/dev/null
  write_report "$id" 3
  if bash "$gate" >"$fixture/fail.log" 2>&1; then
    printf 'Below-minimum %s coverage incorrectly passed\n' "$id" >&2; exit 1
  fi
  write_report "$id" 4
done

# A root migration has no project; migrations within a project are also exempt.
mkdir -p "$fixture/bike-rs/migration/src" "$fixture/bike-rs/bike-core/migration"
printf 'uncovered migration\n' >"$fixture/bike-rs/migration/src/new.rs"
printf 'uncovered migration\n' >"$fixture/bike-rs/bike-core/migration/new.rs"
printf 'TN:\nSF:bike-rs/bike-core/migration/new.rs\nDA:1,0\nLF:1\nLH:0\nend_of_record\n' >>"$fixture/.artifacts/coverage/bike-core/lcov.info"
bash "$gate" >"$fixture/pass.log" 2>&1
jq -e '.total_num_lines == 5' "$fixture/.artifacts/coverage/diff/bike-core.json" >/dev/null
write_report bike-core 4

printf 'modified legacy code\n' >"$fixture/bike-ui/lib/legacy.ts"
printf 'TN:\nSF:bike-ui/lib/legacy.ts\nDA:1,0\nLF:1\nLH:0\nend_of_record\n' >>"$fixture/.artifacts/coverage/bike-ui/lcov.info"
if bash "$gate" >"$fixture/fail.log" 2>&1; then
  printf 'Modified legacy code incorrectly remained exempt\n' >&2; exit 1
fi
printf 'legacy uncovered code\n' >"$fixture/bike-ui/lib/legacy.ts"
write_report bike-ui 4

for id in bike-ui strava-gateway; do
  sed 's#^SF:#SF:/incorrect-root/#' "$fixture/.artifacts/coverage/$id/lcov.info" >"$fixture/invalid-lcov.info"
  mv "$fixture/invalid-lcov.info" "$fixture/.artifacts/coverage/$id/lcov.info"
  if bash "$gate" >"$fixture/fail.log" 2>&1; then
    printf 'Invalid %s source paths silently passed\n' "$id" >&2; exit 1
  fi
  write_report "$id" 4
  : >"$fixture/.artifacts/coverage/$id/lcov.info"
  if bash "$gate" >"$fixture/fail.log" 2>&1; then
    printf 'Empty %s coverage report incorrectly passed\n' "$id" >&2; exit 1
  fi
  write_report "$id" 4
done
bash "$gate" >"$fixture/pass.log" 2>&1

# Native provider summaries exercise the same publishing transformations as CI.
while IFS= read -r project; do
  id="$(jq -r .id <<<"$project")"
  case "$(jq -r .language <<<"$project")" in
    Rust) printf '{"data":[{"files":[],"totals":{"lines":{"count":5,"covered":4,"percent":80}}}]}\n' ;;
    Next.js|Node.js) printf '{"total":{"lines":{"total":5,"covered":4,"pct":80}}}\n' ;;
    Go) printf '{"lines":5,"covered":4,"percent":80}\n' ;;
  esac >"$fixture/.artifacts/coverage/$id/coverage-summary.json"
done < <(jq -c '.[]' "$fixture/coverage-projects.json")
site="$fixture/site"
mkdir -p "$site/coverage"
printf '[{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","created_at":"2026-01-01T00:00:00Z","rust":{"percent":50},"nextjs":{"percent":70}}]\n' >"$site/coverage/history.json"
bash "$repo/scripts/coverage-site.sh" "$site" >"$fixture/pass.log" 2>&1
jq -e '.projects | length == 6' "$site/coverage/latest-summary.json" >/dev/null
jq -e '.projects["strava-gateway"].percent == 80 and .projects["worker"].changed.percent == 80' "$site/coverage/latest-summary.json" >/dev/null
grep -Fq 'Rust workspace (legacy)' "$site/coverage/index.html"
grep -Fq '+10 percentage points' "$site/coverage/index.html"
for id in "${ids[@]}"; do
  test -s "$site/coverage/revisions/$COVERAGE_BASELINE_REVISION/$id/html/index.html"
done
tar -tzf "$site/coverage/revisions/$COVERAGE_BASELINE_REVISION/reports.tar.gz" >"$fixture/archive.txt"
grep -qx 'strava-gateway/lcov.info' "$fixture/archive.txt"
jq '[. + {commit:"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"} | .projects["strava-gateway"].language = "Node.js"]' "$site/coverage/latest-summary.json" >"$site/coverage/history.json"
bash "$repo/scripts/coverage-site.sh" "$site" >"$fixture/pass.log" 2>&1
grep -Fq 'Language changed; new baseline' "$site/coverage/index.html"

# Two real Go modules prove report identity and paths are independent of language.
jq 'map(if .id == "map-renderer" then .language = "Go" | .include = "map-renderer/**/*.go" | .exclude = ["*_test.go", "*.pb.go"] else . end)' "$fixture/coverage-projects.json" >"$fixture/projects.next.json"
mv "$fixture/projects.next.json" "$fixture/coverage-projects.json"
export GOTOOLCHAIN=local GOMODCACHE="$fixture/go-mod" GOCACHE="${GOCACHE:-$repo/.cache/coverage-fixture-go-build}"
for id in strava-gateway map-renderer; do
  printf 'Retired provider report\n' >"$fixture/.artifacts/coverage/$id/html/retired.mjs.html"
  rm -rf -- "${fixture:?}/$id"
  mkdir -p "$fixture/$id"
  printf 'module example.com/coverage-fixture/%s\n\ngo %s\n' "$id" "${GO_VERSION:?Run through mise}" >"$fixture/$id/go.mod"
  cat >"$fixture/$id/math.go" <<'GO'
package fixture

func Sum(a, b int) int {
	return a + b
}
GO
  cat >"$fixture/$id/math_test.go" <<'GO'
package fixture

import "testing"

func TestSum(t *testing.T) {
	if got := Sum(2, 3); got != 5 {
		t.Fatalf("Sum(2, 3) = %d; want 5", got)
	}
}
GO
  bash "$repo/scripts/coverage-go.sh" "$id" >"$fixture/go.log" 2>&1
  awk '/^SF:/' "$fixture/.artifacts/coverage/$id/lcov.info" >>"$fixture/go.log"
  cat "$fixture/.artifacts/coverage/$id/coverage-summary.json" >>"$fixture/go.log"
  grep -Fxq "SF:$id/math.go" "$fixture/.artifacts/coverage/$id/lcov.info"
  jq -e '.lines > 0 and .percent == 100' "$fixture/.artifacts/coverage/$id/coverage-summary.json" >/dev/null
  test -s "$fixture/.artifacts/coverage/$id/html/index.html"
  test ! -e "$fixture/.artifacts/coverage/$id/html/retired.mjs.html"
done
printf 'Coverage fixtures passed: six independent gates, legacy/migrations, historical reports, and two native Go projects.\n'
