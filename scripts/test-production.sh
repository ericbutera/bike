#!/bin/sh
set -eu

repo_dir="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
run_checks() {
  if [ "${1:-k6}" = browser ]; then
    mise --cd "$repo_dir/bike-ui" run test:e2e
  else
    k6 run "$repo_dir/integration-tests/production.js"
  fi
}
if [ -n "${BIKE_SYNTHETIC_KEY:-}" ] && [ -n "${BIKE_API_URL:-}" ] && [ -n "${BIKE_UI_URL:-}" ] && [ -n "${BIKE_PUBLIC_URL:-}" ]; then
  run_checks "${1:-k6}"
  exit
fi

namespace="${BIKE_KUBE_NAMESPACE:-bike}"
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/bike-synthetics.XXXXXX")"
api_forward=''
ui_forward=''
cleanup() {
  [ -z "$api_forward" ] || kill "$api_forward" 2>/dev/null || true
  [ -z "$ui_forward" ] || kill "$ui_forward" 2>/dev/null || true
  rm -rf "$work_dir"
}
trap cleanup EXIT HUP INT TERM

# Secrets stay in the process environment and are never printed or saved.
BIKE_SYNTHETIC_KEY="$(kubectl -n "$namespace" get secret bike-synthetic-credentials -o jsonpath='{.data.key}' | base64 --decode)"
: "${BIKE_SYNTHETIC_KEY:?The production synthetic credential has not been deployed}"
BIKE_PUBLIC_URL="$(kubectl -n "$namespace" get deployment api -o jsonpath='{.spec.template.spec.containers[0].env[?(@.name=="FRONTEND_URL")].value}')"
BIKE_API_URL="http://127.0.0.1:${BIKE_SYNTHETIC_API_PORT:-33000}/api"
BIKE_UI_URL="http://127.0.0.1:${BIKE_SYNTHETIC_UI_PORT:-33001}"
export BIKE_SYNTHETIC_KEY BIKE_PUBLIC_URL BIKE_API_URL BIKE_UI_URL

kubectl -n "$namespace" port-forward --address=127.0.0.1 service/api "${BIKE_SYNTHETIC_API_PORT:-33000}:3000" > "$work_dir/api-forward.log" 2>&1 &
api_forward=$!
kubectl -n "$namespace" port-forward --address=127.0.0.1 service/ui "${BIKE_SYNTHETIC_UI_PORT:-33001}:3000" > "$work_dir/ui-forward.log" 2>&1 &
ui_forward=$!
attempt=0
until curl -fsS "$BIKE_API_URL/health" >/dev/null 2>&1 && curl -fsS "$BIKE_UI_URL" >/dev/null 2>&1; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 20 ] || ! kill -0 "$api_forward" "$ui_forward" 2>/dev/null; then
    echo 'Could not open the production API/UI port forwards' >&2
    exit 1
  fi
  sleep 0.5
done
run_checks "${1:-k6}"
