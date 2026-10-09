#!/usr/bin/env bash
# Free GitHub Pages publishes generated files from a dedicated artifact branch.
set -euo pipefail
# The generated artifact checkout owns its Git metadata even when invoked from a hook.
while IFS= read -r variable; do unset "$variable"; done < <(git rev-parse --local-env-vars)
cd "$(dirname "$0")/.."
if [[ -n "${CI_PIPELINE_EVENT:-}" ]]; then
  [[ "$CI_PIPELINE_EVENT" == push || "$CI_PIPELINE_EVENT" == manual ]]
  [[ "${CI_COMMIT_BRANCH:-}" == main ]]
  [[ "$(git rev-parse HEAD)" == "${CI_COMMIT_SHA:?CI source revision is required}" ]]
fi
repository="ericbutera/bike"
remote="https://github.com/$repository.git"
latest_main="$(git ls-remote "$remote" refs/heads/main | cut -f1)"
if [[ "$latest_main" != "$(git rev-parse HEAD)" ]]; then
  printf 'Skipping publication: this revision is no longer remote main.\n'
  exit 0
fi
site="$(mktemp -d "${TMPDIR:-/tmp}/bike-coverage-pages.XXXXXX")"
trap 'rm -rf -- "$site"' EXIT

if git ls-remote --exit-code --heads "$remote" refs/heads/gh-pages >"$site/branch.txt"; then
  git clone --quiet --depth=1 --single-branch --branch gh-pages "$remote" "$site/repo"
else
  status=$?
  [[ "$status" == 2 ]]
  git init --quiet --initial-branch=gh-pages "$site/repo"
  git -C "$site/repo" remote add origin "$remote"
fi
mise run coverage:site -- "$site/repo"
git -C "$site/repo" config user.name 'Bike coverage'
git -C "$site/repo" config user.email 'coverage@users.noreply.github.com'
git -C "$site/repo" add .
git -C "$site/repo" commit --quiet -m "docs: coverage reports for $(git rev-parse --short=12 HEAD)"
git -C "$site/repo" -c credential.helper= -c 'credential.helper=!gh auth git-credential' \
  push --quiet origin gh-pages

if [[ "$(gh api "repos/$repository" --jq .has_pages)" == false ]]; then
  printf '{"build_type":"legacy","source":{"branch":"gh-pages","path":"/"}}\n' \
    | gh api --method POST "repos/$repository/pages" --input - >/dev/null
fi
url="$(gh api "repos/$repository/pages" --jq .html_url)"
printf 'Browsable coverage reports: %scoverage/\n' "$url"
