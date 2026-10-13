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

attributes=$(temporal --address "$TEMPORAL_ADDRESS" --namespace "$TEMPORAL_NAMESPACE" --output json operator search-attribute list)
for name in BikeCustomerId BikeUserId StravaAthleteId StravaActivityId StravaEventId CorrelationId; do
  if printf '%s\n' "$attributes" | grep -Fq "\"$name\": \"INDEXED_VALUE_TYPE_KEYWORD\""; then
    continue
  fi
  if printf '%s\n' "$attributes" | grep -Fq "\"$name\":"; then
    printf 'Search attribute %s must be Keyword\n' "$name" >&2
    exit 1
  fi
  temporal --address "$TEMPORAL_ADDRESS" --namespace "$TEMPORAL_NAMESPACE" operator search-attribute create --name "$name" --type Keyword
done
