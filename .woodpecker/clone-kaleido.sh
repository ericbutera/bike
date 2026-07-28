#!/bin/sh
set -e

if [ ! -d kaleido/.git ]; then
  git clone --depth=1 https://github.com/ericbutera/kaleido kaleido
fi

# Keep kaleido inside the Docker context so cargo-chef and the final image build
# use the same path-patched dependency graph that Rust tests use in CI.
sed -i 's#../kaleido/#kaleido/#g' Cargo.toml
