#!/usr/bin/env bash
# Free GitHub Pages publishes generated files from a dedicated artifact branch.
set -euo pipefail
umask 077
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
: "${COVERAGE_PUBLISH_KEY:?A dedicated repository deploy key is required}"
: "${COVERAGE_GITHUB_SSH_HOST_KEY:?The pinned GitHub SSH host key is required}"
printf '%s\n' "$COVERAGE_PUBLISH_KEY" >"$site/key"
printf '[ssh.github.com]:443 %s\n' "$COVERAGE_GITHUB_SSH_HOST_KEY" >"$site/known_hosts"
export GIT_SSH_COMMAND="ssh -i $site/key -o IdentitiesOnly=yes -o UserKnownHostsFile=$site/known_hosts -o StrictHostKeyChecking=yes"

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
git -C "$site/repo" push --quiet "ssh://git@ssh.github.com:443/$repository.git" gh-pages
printf 'Browsable coverage reports: %s/\n' "${COVERAGE_SITE_URL:?Coverage site URL is required}"
