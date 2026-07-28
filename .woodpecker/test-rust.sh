#!/bin/sh
set -e

if [ -n "${CARGO_HOME:-}" ]; then
  mkdir -p "$CARGO_HOME"
fi

if [ -n "${RUSTUP_HOME:-}" ]; then
  mkdir -p "$RUSTUP_HOME"
fi

if [ -n "${CARGO_TARGET_DIR:-}" ]; then
  mkdir -p "$CARGO_TARGET_DIR"
fi

cargo fmt --all --check
cargo test --workspace
