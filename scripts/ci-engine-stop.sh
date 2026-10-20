#!/usr/bin/env bash
# Workflow cleanup for the disposable CI Docker engine.
finish_ci_engine() {
  local engine_attempt
  mkdir -p "$BIKE_E2E_ENGINE_STATE_DIR"
  touch "$BIKE_E2E_ENGINE_STATE_DIR/stop"
  for ((engine_attempt=0; engine_attempt<30; engine_attempt++)); do
    if [[ -f "$BIKE_E2E_ENGINE_STATE_DIR/stopped" ]]; then
      test "$(cat "$BIKE_E2E_ENGINE_STATE_DIR/stopped")" = 0
      return
    fi
    sleep 1
  done
  printf 'Job-local Docker engine did not stop cleanly\n' >&2
  return 1
}


# The workflow owns shutdown even when checks fail before image acceptance.
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  set -euo pipefail
  : "${BIKE_E2E_ENGINE_STATE_DIR:?CI must select the job-local engine directory}"
  finish_ci_engine
fi
