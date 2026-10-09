#!/usr/bin/env bash
set -euo pipefail
test -s .artifacts/e2e-images.env
[[ "${BIKE_TESTED_REVISION:?}" == "${CI_COMMIT_SHA:?}" ]]
# Credentials remain in the environment, outside Git's URL and logs.
export GIT_CONFIG_COUNT=1
export GIT_CONFIG_KEY_0=http.https://github.com/.extraheader
GIT_CONFIG_VALUE_0="AUTHORIZATION: basic $(printf 'x-access-token:%s' "${GITHUB_TOKEN:?}" | base64 | tr -d '\n')"
export GIT_CONFIG_VALUE_0
git clone --quiet "https://github.com/${DEPLOYMENT_REPO:?}" .artifacts/deployment
unset GIT_CONFIG_COUNT GIT_CONFIG_KEY_0 GIT_CONFIG_VALUE_0
mise trust --yes .artifacts/deployment/mise.toml
mise --cd .artifacts/deployment run ci:deploy
