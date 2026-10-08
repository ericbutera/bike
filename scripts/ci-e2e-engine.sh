#!/bin/sh
# Own the job engine's lifetime; a successful gate stops it before CI teardown.
set -eu
cd "$(dirname "$0")/.."
: "${BIKE_E2E_ENGINE_STATE_DIR:?The job must select its private engine directory}"
# Containerd must create searchable container root filesystems for non-root
# application users. Restrict the engine directory itself instead.
umask 022
mkdir -p "$BIKE_E2E_ENGINE_STATE_DIR"
chmod 700 "$BIKE_E2E_ENGINE_STATE_DIR"
export TINI_SUBREAPER=1

containerd --config scripts/ci-e2e-containerd.toml &
containerd_pid=$!
daemon_pid=''
cleanup() {
  result=$1
  trap - EXIT
  if [ -n "$daemon_pid" ] && kill -0 "$daemon_pid" 2>/dev/null; then
    kill -TERM "$daemon_pid"
    wait "$daemon_pid" || result=1
  fi
  if kill -0 "$containerd_pid" 2>/dev/null; then kill -TERM "$containerd_pid"; fi
  wait "$containerd_pid" || result=1
  printf '%s\n' "$result" >"$BIKE_E2E_ENGINE_STATE_DIR/stopped.tmp"
  mv "$BIKE_E2E_ENGINE_STATE_DIR/stopped.tmp" "$BIKE_E2E_ENGINE_STATE_DIR/stopped"
  exit "$result"
}
trap 'cleanup "$?"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

attempt=0
while [ ! -S /var/run/docker/containerd/containerd.sock ]; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 10 ]; then
    printf 'Job-local containerd did not become ready\n' >&2
    exit 1
  fi
  kill -0 "$containerd_pid"
  sleep 1
done
dockerd-entrypoint.sh dockerd \
  --host="unix://$BIKE_E2E_ENGINE_STATE_DIR/docker.sock" \
  --containerd=/var/run/docker/containerd/containerd.sock "$@" &
daemon_pid=$!

while kill -0 "$daemon_pid" 2>/dev/null; do
  kill -0 "$containerd_pid"
  if [ -f "$BIKE_E2E_ENGINE_STATE_DIR/stop" ]; then cleanup 0; fi
  sleep 1
done
# An unsolicited clean daemon exit also fails the gate.
wait "$daemon_pid" || exit "$?"
printf 'Job-local Docker daemon stopped before the gate finished\n' >&2
exit 1
