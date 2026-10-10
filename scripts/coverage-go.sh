#!/usr/bin/env bash
# Native Go tests/HTML and the pinned converter produce the common LCOV report.
set -euo pipefail
cd "${COVERAGE_REPO_DIR:-$(dirname "$0")/..}"
repo="$PWD"
id="${1:?Coverage project ID is required}"
source="$(jq -er --arg id "$id" '.[] | select(.id == $id and .language == "Go") | .source' coverage-projects.json)"
test -f "$source/go.mod"
output="$repo/.artifacts/coverage/$id"
rm -rf -- "$output"
mkdir -p "$output/html"
cd "$source"
# Instrument all handwritten packages, including packages without tests.
packages="$(go list ./... | awk '!/\/gen\// { printf "%s%s", separator, $0; separator = "," }')"
test -n "$packages"
# Coverage remains unit-only even when a developer has configured PostgreSQL checks.
env -u TEST_DATABASE_URL go test -p 1 \
  -covermode=atomic -coverpkg="$packages" -coverprofile="$output/coverage.out" ./...
go tool cover -html="$output/coverage.out" -o "$output/html/index.html"
go tool cover -func="$output/coverage.out" >"$output/statements.txt"
gcov2lcov \
  -use-absolute-source-path -infile="$output/coverage.out" -outfile="$output/lcov.info" \
  2>"$output/conversion.log"
# Upstream logs unresolved source paths as warnings; reject partial conversions.
if [[ -s "$output/conversion.log" ]]; then
  cat "$output/conversion.log" >&2
  exit 1
fi
# Go's resolved module directory also handles OS aliases such as /var -> /private/var.
module_dir="$(go list -m -f '{{.Dir}}')"
COVERAGE_REPO_DIR="${module_dir%/"$source"}" \
  bash "$repo/scripts/coverage-normalize.sh" "$output/lcov.info"
jq -Rn '
  reduce inputs as $line ({lines: 0, covered: 0};
    if $line | startswith("LF:") then .lines += ($line[3:] | tonumber)
    elif $line | startswith("LH:") then .covered += ($line[3:] | tonumber)
    else . end)
  | .percent = (if .lines == 0 then 0 else 100 * .covered / .lines end)
' "$output/lcov.info" >"$output/coverage-summary.json"
test -s "$output/lcov.info"
printf 'Go HTML report: %s/html/index.html\n' "$output"
