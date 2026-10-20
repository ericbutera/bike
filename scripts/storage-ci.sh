#!/usr/bin/env bash
# Fail closed on the existing collector's capacity and freshness measurements.
set -euo pipefail
: "${BIKE_STORAGE_METRICS_URL:?CI must supply its storage metrics endpoint}"

query() {
  curl --fail --silent --show-error --max-time 30 --get \
    --data-urlencode "query=$1" "$BIKE_STORAGE_METRICS_URL/api/v1/query" |
    jq --exit-status '
      if .status == "success" and .data.resultType == "vector" and
         ((.warnings // []) | length) == 0
      then .data.result else error("Storage metrics query failed") end'
}

# The ANDs retain only successful scans from a collector updated within 10m.
usage="$(query 'nibelheim_storage_path_used_bytes{kind="pvc",persistentvolumeclaim=~"bike-buildkit-cache|woodpecker-cache|registry-data"} and (nibelheim_storage_path_scan_success == 1) and on(node) (time() - nibelheim_storage_collection_timestamp_seconds < 600)')"
now="$(date +%s)"
for claim in bike-buildkit-cache woodpecker-cache registry-data; do
  namespace=woodpecker
  budget=$((80 * 1024 * 1024 * 1024))
  if [[ "$claim" == registry-data ]]; then
    namespace=registry
    budget=$((40 * 1024 * 1024 * 1024))
  fi
  observation="$(jq --exit-status --arg claim "$claim" --arg namespace "$namespace" --argjson now "$now" '
    map(select(.metric.persistentvolumeclaim == $claim and .metric.namespace == $namespace)) |
    if length != 1 then error("Missing or ambiguous storage scan: " + $claim)
    else .[0] end |
    if (.value[1] | test("^[0-9]{1,15}$")) and (.value[0] >= $now - 120) and
       (.value[0] <= $now + 60) then . else error("Invalid or stale storage scan") end' <<<"$usage")"
  used="$(jq --raw-output '.value[1]' <<<"$observation")"
  mount="$(jq --compact-output '.metric.mountpoint' <<<"$observation")"
  node="$(jq --compact-output '.metric.node' <<<"$observation")"
  # Match the measured PVC's filesystem and node, not an unrelated boot disk.
  free="$(query "node_filesystem_avail_bytes{job=\"node-exporter\",mountpoint=$mount} * on(instance) group_left(node) label_replace(node_uname_info{job=\"node-exporter\",nodename=$node}, \"node\", \"\$1\", \"nodename\", \"(.*)\")")"
  available="$(jq --exit-status --raw-output --argjson now "$now" '
    if length != 1 then error("Missing or ambiguous filesystem capacity") else .[0] end |
    if (.value[1] | test("^[0-9]{1,15}$")) and (.value[0] >= $now - 120) and
       (.value[0] <= $now + 60) then .value[1] else error("Invalid or stale filesystem capacity") end' <<<"$free")"
  printf 'CI storage: %s used=%s budget=%s available=%s bytes.\n' "$claim" "$used" "$budget" "$available"
  if (( used > budget || available < 80 * 1024 * 1024 * 1024 )); then
    printf 'Storage capacity gate refused work. Run coordinated retention and verify space before retrying.\n' >&2
    exit 1
  fi
done
