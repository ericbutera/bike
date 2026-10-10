#!/usr/bin/env bash
# Render service reports from the single collected workspace measurement.
set -euo pipefail
cd "$(dirname "$0")/.."
repo="$PWD"
while IFS= read -r project; do
  id="$(jq -r .id <<<"$project")"
  other_crates="$(jq -r --arg id "$id" '[.[] | select(.language == "Rust" and .id != $id) | .source | split("/")[-1]] + ["migration"] | join("|")' coverage-projects.json)"
  output="$repo/.artifacts/coverage/$id"
  rm -rf -- "$output"
  mkdir -p "$output"
  for format in html lcov json; do
    case "$format" in
      html) args=(--html --output-dir "$output") ;;
      lcov) args=(--lcov --output-path "$output/lcov.info") ;;
      json) args=(--json --summary-only --output-path "$output/coverage-summary.json") ;;
    esac
    mise --cd bike-rs exec rust protoc aqua:taiki-e/cargo-llvm-cov -- \
      cargo llvm-cov report --ignore-filename-regex "(^|/)bike-rs/($other_crates)/" "${args[@]}"
  done
  bash scripts/coverage-normalize.sh "$output/lcov.info"
done < <(jq -c '.[] | select(.language == "Rust")' coverage-projects.json)
