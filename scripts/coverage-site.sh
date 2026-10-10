#!/usr/bin/env bash
# Service identities survive language changes; native reports stay separate.
set -euo pipefail
cd "${COVERAGE_REPO_DIR:-$(dirname "$0")/..}"
site="${1:-.artifacts/coverage-site}"
revision="$(git rev-parse HEAD)"
: "${COVERAGE_MINIMUM:?Run through mise run coverage:site}"
report="${site}/coverage/revisions/${revision}"
mkdir -p "$report"
summaries="$report/projects.jsonl"
: >"$summaries"
projects=()

while IFS= read -r project; do
  id="$(jq -r .id <<<"$project")"
  language="$(jq -r .language <<<"$project")"
  input=".artifacts/coverage/$id"
  for artifact in html/index.html lcov.info coverage-summary.json; do test -s "$input/$artifact"; done
  test -s ".artifacts/coverage/diff/$id.json"
  mkdir -p "$report/$id"
  cp -R "$input/html" "$report/$id/"
  cp "$input/lcov.info" "$input/coverage-summary.json" "$report/$id/"
  for extra in coverage.out statements.txt; do
    if [[ -f "$input/$extra" ]]; then cp "$input/$extra" "$report/$id/"; fi
  done
  case "$language" in
    Rust)
      jq -e 'all(.data[].files[].filename; contains("/migration/") | not)' "$input/coverage-summary.json" >/dev/null
      summary="$(jq -c '.data[0].totals.lines | {lines:.count, covered, percent}' "$input/coverage-summary.json")"
      ;;
    Next.js|Node.js)
      summary="$(jq -c '.total.lines | {lines:.total, covered, percent:.pct}' "$input/coverage-summary.json")"
      ;;
    Go) summary="$(jq -c '{lines, covered, percent}' "$input/coverage-summary.json")" ;;
    *) printf 'Unsupported coverage provider: %s\n' "$language" >&2; exit 1 ;;
  esac
  jq -n --argjson project "$project" --argjson summary "$summary" \
    --slurpfile diff ".artifacts/coverage/diff/$id.json" '
    $project + $summary + {changed: {
      lines: $diff[0].total_num_lines,
      covered: $diff[0].total_num_lines - $diff[0].total_num_violations,
      percent: $diff[0].total_percent_covered
    }}' >>"$summaries"
  projects+=("$id")
done < <(jq -c '.[]' coverage-projects.json)
test "${#projects[@]}" -gt 0
cp -R .artifacts/coverage/diff "$report/"
tar -czf "$report/reports.tar.gz" -C "$report" "${projects[@]}" diff

jq -n --arg commit "$revision" --arg created "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg base "$(cat .artifacts/coverage/diff/base-revision.txt)" \
  --arg pipeline "${CI_PIPELINE_URL:-}" --argjson minimum "$COVERAGE_MINIMUM" \
  --slurpfile projects "$summaries" '
  {commit: $commit, created_at: $created, base_revision: $base, pipeline_url: $pipeline,
   minimum_changed_line_percent: $minimum,
   projects: ($projects | map({key:.id, value:del(.id, .include, .exclude)}) | from_entries)}' \
  >"$report/summary.json"
rm "$summaries"

history="$site/coverage/history.json"
if [[ ! -f "$history" ]]; then printf '[]\n' >"$history"; fi
jq --slurpfile current "$report/summary.json" \
  '[$current[0]] + map(select(.commit != $current[0].commit)) | .[:10]' "$history" \
  >"$site/coverage/history.next.json"
mv "$site/coverage/history.next.json" "$history"
cp "$report/summary.json" "$site/coverage/latest-summary.json"

for directory in "$site"/coverage/revisions/*; do
  name="${directory##*/}"
  if ! jq -e --arg commit "$name" 'any(.[]; .commit == $commit)' "$history" >/dev/null; then
    rm -rf -- "$directory"
  fi
done

delta() {
  jq -r --arg id "$1" '
    def project:
      .projects[$id] // (if $id == "bike-ui" and .nextjs then .nextjs + {language:"Next.js"} else null end);
    (.[0] | project) as $current | (.[1] | project) as $previous |
    if $previous == null then "First service report"
    elif $current.language != $previous.language then "Language changed; new baseline"
    else ($current.percent - $previous.percent | . * 100 | round / 100) as $delta |
      "\(if $delta > 0 then "+" else "" end)\($delta) percentage points" end
  ' "$history"
}

{
  cat <<HTML
<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Bike test coverage</title><style>
body{font:16px system-ui,sans-serif;max-width:1200px;margin:40px auto;padding:0 20px;color:#172033;background:#f8fafc}
table{border-collapse:collapse;width:100%;background:white;margin:24px 0}th,td{padding:12px;text-align:left;border-bottom:1px solid #dbe2ea}
a{color:#0759ad}code{overflow-wrap:anywhere}small{color:#526174}h1{margin-bottom:8px}.table{overflow-x:auto}
</style></head><body>
<h1>Bike test coverage</h1>
<p>Unit tests only. Added and changed executable lines require ${COVERAGE_MINIMUM}% coverage independently for each service or shared project. Untouched existing code has no minimum. Migrations are excluded.</p>
<p>Source revision: <a href="https://github.com/ericbutera/bike/commit/${revision}"><code>${revision}</code></a></p>
<div class="table"><table><thead><tr><th>Service / project</th><th>Overall lines</th><th>Change from prior report</th><th>Changed lines</th><th>Reports</th></tr></thead><tbody>
HTML
  for id in "${projects[@]}"; do
    jq -r --arg id "$id" --arg revision "$revision" --arg delta "$(delta "$id")" '
      .projects[$id] |
      "<tr><td>\(.name | @html)<br><small>\(.language | @html)</small></td><td>\(.percent * 100 | round / 100)% (\(.covered)/\(.lines))</td><td>\($delta | @html)</td><td>\(if .changed.lines == 0 then "No changed executable lines" else "\(.changed.percent * 100 | round / 100)% (\(.changed.covered)/\(.changed.lines) lines)" end)</td><td><a href=\"revisions/\($revision)/\($id)/html/index.html\">Full report</a> · <a href=\"revisions/\($revision)/diff/\($id).html\">Changed lines</a></td></tr>"
    ' "$report/summary.json"
  done
  cat <<HTML
</tbody></table></div>
<p>Overall coverage is informational. Shared Bike core has its own report rather than being counted twice in API and worker. Coverage records executed lines; meaningful happy-path assertions still require review.</p>
<p>Go's native HTML shows statement coverage; the table and changed-line gate use LCOV lines. A language change starts a new trend baseline for that service.</p>
<p><a href="revisions/${revision}/reports.tar.gz">Download HTML, LCOV, JSON, and changed-line reports</a> · <a href="latest-summary.json">Current summary JSON</a> · <a href="history.json">History JSON</a></p>
<h2>Recent reports</h2><div class="table"><table><thead><tr><th>Revision</th><th>Collected (UTC)</th><th>Service reports</th></tr></thead><tbody>
HTML
  jq -r '
    .[] | .commit as $revision |
    (if .projects then .projects | to_entries | map({id:.key, name:.value.name, percent:.value.percent})
     else [{id:"rust", name:"Rust workspace (legacy)", percent:.rust.percent}, {id:"nextjs", name:"Bike UI", percent:.nextjs.percent}] end) as $projects |
    "<tr><td><a href=\"https://github.com/ericbutera/bike/commit/\($revision)\"><code>\($revision[0:12])</code></a></td><td>\(.created_at | @html)</td><td>\($projects | map("<a href=\"revisions/\($revision)/\(.id)/html/index.html\">\(.name | @html): \(.percent * 100 | round / 100)%</a>") | join(" · "))</td></tr>"
  ' "$history"
  printf '</tbody></table></div><p><small>Ten recent revisions are retained. Compare reports from the same platform, language, and suite for reliable trends.</small></p></body></html>\n'
} >"$site/coverage/index.html"
printf '<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Bike reports</title></head><body><a href="coverage/">Bike test coverage</a></body></html>\n' >"$site/index.html"
touch "$site/.nojekyll"
printf 'Coverage site assembled at %s/coverage/index.html\n' "$site"
