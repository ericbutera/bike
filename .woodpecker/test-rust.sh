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

RUST_VERSION="$(sed -n 's/^rust = "\(.*\)"$/\1/p' mise.toml)"
if [ -z "$RUST_VERSION" ]; then
  echo "Unable to read Rust version from mise.toml" >&2
  exit 1
fi

rustup toolchain install "$RUST_VERSION" --profile minimal --component rustfmt --component clippy

cargo +"$RUST_VERSION" fmt --all --check
cargo +"$RUST_VERSION" clippy --workspace --all-targets -- -D warnings
cargo +"$RUST_VERSION" test --workspace
