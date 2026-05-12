#!/bin/sh
set -e

MANUAL="${CI_PIPELINE_EVENT}"
BEFORE="${CI_COMMIT_BEFORE}"
FALLBACK_DEPLOY_ALL=false

if [ -n "$BEFORE" ] && [ "$BEFORE" != "0000000000000000000000000000000000000000" ]; then
  git fetch --depth=1 origin "$BEFORE" 2>/dev/null || true
  CHANGED=$(git diff --name-only "$BEFORE" "${CI_COMMIT_SHA}" 2>/dev/null || true)
  if [ -z "$CHANGED" ]; then
    FALLBACK_DEPLOY_ALL=true
  fi
else
  FALLBACK_DEPLOY_ALL=true
fi

API_TAG=""
WORKER_TAG=""
UI_TAG=""

if [ "$MANUAL" = "manual" ] || [ "$FALLBACK_DEPLOY_ALL" = "true" ] || echo "$CHANGED" | grep -qE "^(api/|migration/|Cargo\.toml|Cargo\.lock|\.woodpecker/)"; then
  API_TAG="${CI_COMMIT_SHA}"
fi
if [ "$MANUAL" = "manual" ] || [ "$FALLBACK_DEPLOY_ALL" = "true" ] || echo "$CHANGED" | grep -qE "^(api/|worker/|migration/|Cargo\.toml|Cargo\.lock|\.woodpecker/)"; then
  WORKER_TAG="${CI_COMMIT_SHA}"
fi
if [ "$MANUAL" = "manual" ] || [ "$FALLBACK_DEPLOY_ALL" = "true" ] || echo "$CHANGED" | grep -qE "^(ui-next/|\.woodpecker/)"; then
  UI_TAG="${CI_COMMIT_SHA}"
fi

if [ -z "$API_TAG" ] && [ -z "$WORKER_TAG" ] && [ -z "$UI_TAG" ]; then
  echo "No deployable changes detected, skipping deploy"
  exit 0
fi

git clone "https://x-access-token:${GITHUB_TOKEN}@github.com/${PULUMI_IAC_REPO}" /tmp/pulumi-iac
cd /tmp/pulumi-iac/bike
pulumi stack select ericbutera/bike/bike --non-interactive

if [ -n "$API_TAG" ];    then pulumi config set bike:apiTag    "$API_TAG";    fi
if [ -n "$WORKER_TAG" ]; then pulumi config set bike:workerTag "$WORKER_TAG"; fi
if [ -n "$UI_TAG" ];     then pulumi config set bike:uiTag     "$UI_TAG";     fi

pulumi up --yes --skip-preview
