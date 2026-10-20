#!/usr/bin/env bash
# Admission for local image builds; Docker protects all container references.
set -euo pipefail
case "${1:---prune}" in
  --prune) docker image prune --all --force --filter label=dev.bike.storage=disposable-image --filter until=24h ;;
  --check) ;;
  *) printf 'Usage: %s [--prune|--check]\n' "$0" >&2; exit 2 ;;
esac
ids="$(docker image ls --quiet --no-trunc | sort -u)"
image_bytes=0
if [[ -n "$ids" ]]; then
  # xargs handles a large inventory without exceeding the argument-size limit.
  sizes="$(printf '%s\n' "$ids" | xargs docker image inspect --format '{{.Size}}')"
  image_bytes="$(printf '%s\n' "$sizes" | awk '
    !/^[0-9]+$/ {exit 1}
    {total += $1}
    END {printf "%.0f\n", total}')"
fi
if [[ ! "$image_bytes" =~ ^[0-9]{1,15}$ ]]; then
  printf "Invalid Docker image byte count; refusing build.\n" >&2
  exit 1
fi
available_kib="$(df -Pk /cache | awk 'NR == 2 {print $4}')"
if [[ ! "$available_kib" =~ ^[0-9]{1,15}$ ]]; then
  printf 'Cannot verify Docker storage free space; refusing build.\n' >&2
  exit 1
fi
printf 'Local storage: images=%s bytes (32 GiB budget), available=%s KiB (64 GiB minimum).\n' "$image_bytes" "$available_kib"
if (( image_bytes > 32 * 1024 * 1024 * 1024 )); then
  printf 'Image budget exceeded. Review unused images and stopped containers before rebuilding.\n' >&2
  exit 1
fi
if (( available_kib < 64 * 1024 * 1024 )); then
  printf 'Insufficient Docker disk space; reclaim disposable storage before rebuilding.\n' >&2
  exit 1
fi
