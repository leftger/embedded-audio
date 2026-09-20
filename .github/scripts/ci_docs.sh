#!/usr/bin/env bash
# Build docs with warnings denied (broken links, missing docs, etc.).
set -euo pipefail

export RUSTDOCFLAGS="${RUSTDOCFLAGS:--D warnings}"

# The firmware example targets thumbv8m and only builds with an explicit
# `--target`, so keep it out of the host doc build.
cargo doc --workspace --exclude stm32wba65ri-pwm-audio --no-deps --all-features

echo '<!DOCTYPE html><html><head><meta http-equiv="refresh" content="0; url=embedded_audio/index.html"></head></html>' > target/doc/index.html
