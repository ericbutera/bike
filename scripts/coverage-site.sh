#!/usr/bin/env bash
# Assemble static native reports; JSON transformations use the providers' schemas.
set -euo pipefail
cd "$(dirname "$0")/.."
site="${1:-.artifacts/coverage-site}"
revision="$(git rev-parse HEAD)"
: "${COVERAGE_MINIMUM:?Run through mise run coverage:site}"
report="${site}/coverage/revisions/${revision}"
mkdir -p "$report"

for suite in rust nextjs; do
  for artifact in html/index.html lcov.info coverage-summary.json; do
    test -s ".artifacts/coverage/$suite/$artifact"
  done
  cp -R ".artifacts/coverage/$suite" "$report/"
  test -s ".artifacts/coverage/diff/$suite.json"
done
jq -e 'all(.data[].files[].filename; contains("/migration/") | not)' \
  .artifacts/coverage/rust/coverage-summary.json >/dev/null
cp -R .artifacts/coverage/diff "$report/"
tar -czf "$report/reports.tar.gz" -C .artifacts/coverage rust nextjs diff

jq -n --arg commit "$revision" --arg created "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg base "$(cat .artifacts/coverage/diff/base-revision.txt)" \
  --arg pipeline "${CI_PIPELINE_URL:-}" --argjson minimum "$COVERAGE_MINIMUM" \
  --slurpfile rust .artifacts/coverage/rust/coverage-summary.json \
  --slurpfile nextjs .artifacts/coverage/nextjs/coverage-summary.json \
  --slurpfile rust_diff .artifacts/coverage/diff/rust.json \
  --slurpfile nextjs_diff .artifacts/coverage/diff/nextjs.json '
  def changed($data): {lines: $data.total_num_lines, covered: $data.total_num_lines - $data.total_num_violations, percent: $data.total_percent_covered};
  {commit: $commit, created_at: $created, base_revision: $base, pipeline_url: $pipeline,
   minimum_changed_line_percent: $minimum,
   rust: {lines: $rust[0].data[0].totals.lines.count, covered: $rust[0].data[0].totals.lines.covered,
          percent: $rust[0].data[0].totals.lines.percent, changed: changed($rust_diff[0])},
   nextjs: {lines: $nextjs[0].total.lines.total, covered: $nextjs[0].total.lines.covered,
            percent: $nextjs[0].total.lines.pct, changed: changed($nextjs_diff[0])}}' \
  >"$report/summary.json"

history="$site/coverage/history.json"
if [[ ! -f "$history" ]]; then printf '[]\n' >"$history"; fi
jq --slurpfile current "$report/summary.json" \
  '[$current[0]] + map(select(.commit != $current[0].commit)) | .[:10]' "$history" \
  >"$site/coverage/history.next.json"
mv "$site/coverage/history.next.json" "$history"
cp "$report/summary.json" "$site/coverage/latest-summary.json"

# Keep ten browsable revisions. Git retains ordinary append-only publication history.
for directory in "$site"/coverage/revisions/*; do
  name="${directory##*/}"
  if ! jq -e --arg commit "$name" 'any(.[]; .commit == $commit)' "$history" >/dev/null; then
    rm -rf -- "$directory"
  fi
done

percent() { jq -r "$1 | . * 100 | round / 100 | tostring" "$report/summary.json"; }
changed() {
  jq -r --arg suite "$1" \
    '.[ $suite ].changed | if .lines == 0 then "No changed executable lines" else "\(.percent)% (\(.covered)/\(.lines) lines)" end' \
    "$report/summary.json"
}
delta() {
  jq -r --arg suite "$1" \
    'if length < 2 then "First report" else (.[0][$suite].percent - .[1][$suite].percent | . * 100 | round / 100) as $delta | "\(if $delta > 0 then "+" else "" end)\($delta) percentage points" end' "$history"
}
{
  cat <<HTML
<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Bike test coverage</title><style>
body{font:16px system-ui,sans-serif;max-width:1100px;margin:40px auto;padding:0 20px;color:#172033;background:#f8fafc}
table{border-collapse:collapse;width:100%;background:white;margin:24px 0}th,td{padding:12px;text-align:left;border-bottom:1px solid #dbe2ea}
a{color:#0759ad}code{overflow-wrap:anywhere}small{color:#526174}h1{margin-bottom:8px}
</style></head><body>
<h1>Bike test coverage</h1>
<p>Unit tests only. Added and changed executable lines require ${COVERAGE_MINIMUM}% coverage per project. Untouched existing code has no minimum. Rust migrations are excluded.</p>
<p>Source revision: <a href="https://github.com/ericbutera/bike/commit/${revision}"><code>${revision}</code></a></p>
<table><thead><tr><th>Project</th><th>Overall lines</th><th>Change from prior report</th><th>Changed lines</th><th>Reports</th></tr></thead><tbody>
<tr><td>Rust</td><td>$(percent '.rust.percent')%</td><td>$(delta rust)</td><td>$(changed rust)</td><td><a href="revisions/${revision}/rust/html/index.html">Full report</a> · <a href="revisions/${revision}/diff/rust.html">Changed lines</a></td></tr>
<tr><td>Next.js</td><td>$(percent '.nextjs.percent')%</td><td>$(delta nextjs)</td><td>$(changed nextjs)</td><td><a href="revisions/${revision}/nextjs/html/index.html">Full report</a> · <a href="revisions/${revision}/diff/nextjs.html">Changed lines</a></td></tr>
</tbody></table>
<p>Overall coverage is informational. Coverage records executed lines; meaningful happy-path assertions still require review.</p>
<p><a href="revisions/${revision}/reports.tar.gz">Download HTML, LCOV, JSON, and changed-line reports</a> · <a href="latest-summary.json">Current summary JSON</a> · <a href="history.json">History JSON</a></p>
<h2>Recent reports</h2><table><thead><tr><th>Revision</th><th>Collected (UTC)</th><th>Rust lines</th><th>Next.js lines</th><th>Reports</th></tr></thead><tbody>
HTML
  jq -r '.[] | "<tr><td><a href=\"https://github.com/ericbutera/bike/commit/\(.commit)\"><code>\(.commit[0:12])</code></a></td><td>\(.created_at | @html)</td><td>\(.rust.percent * 100 | round / 100)%</td><td>\(.nextjs.percent)%</td><td><a href=\"revisions/\(.commit)/rust/html/index.html\">Rust</a> · <a href=\"revisions/\(.commit)/nextjs/html/index.html\">Next.js</a></td></tr>"' "$history"
  printf '</tbody></table><p><small>Ten recent revisions are retained. Compare reports from the same platform and suite for reliable trends.</small></p></body></html>\n'
} >"$site/coverage/index.html"
printf '<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Bike reports</title></head><body><a href="coverage/">Bike test coverage</a></body></html>\n' >"$site/index.html"
touch "$site/.nojekyll"
printf 'Coverage site assembled at %s/coverage/index.html\n' "$site"
