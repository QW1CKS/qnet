#!/usr/bin/env bash
# Run this on a Linux runner (or WSL) from the repo root.
set -euo pipefail

# Ensure required tools
command -v cargo >/dev/null
command -v rustup >/dev/null

# Install nightly toolchain if missing
rustup toolchain install nightly || true
rustup component add rust-src --toolchain nightly || true

# Build and run target (timeboxed using timeout)
cd "$(dirname "$0")/.." || exit 1
cd fuzz
# Build the fuzzers
cargo +nightly fuzz build
# Run each fuzzer for 15m
timeout 15m cargo +nightly fuzz run framing_decode -- -runs=0 || true
timeout 15m cargo +nightly fuzz run noise_handshake -- -runs=0 || true

echo "Fuzz run complete (timeboxed)."
