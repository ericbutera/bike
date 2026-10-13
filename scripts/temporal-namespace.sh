#!/bin/sh
set -eu

temporal --address "${TEMPORAL_ADDRESS:?}" operator cluster health

if result=$(temporal --address "$TEMPORAL_ADDRESS" --namespace "${TEMPORAL_NAMESPACE:?}" operator namespace create --retention 7d 2>&1); then
  printf '%s\n' "$result"
else
  case "$result" in
    *AlreadyExists*|*already\ exists*)
      temporal --address "$TEMPORAL_ADDRESS" --namespace "$TEMPORAL_NAMESPACE" operator namespace update --retention 7d
      ;;
    *)
      printf '%s\n' "$result" >&2
      exit 1
      ;;
  esac
fi
