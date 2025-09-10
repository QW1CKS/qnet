#!/usr/bin/env bash
# Linux perf run helper for T6.6
# Usage: ./perf-run-linux.sh [--features "perf-bench,quic"]
set -euo pipefail

FEATURES=${1:-"perf-bench"}

echo "[perf] Collecting hardware profile..."
mkdir -p artifacts
{
  echo "# Host"
  uname -a
  echo
  echo "# CPU"
  lscpu || true
  echo
  echo "# Memory"
  free -h || true
} > artifacts/linux-hw-profile.txt

echo "[perf] Running benches with features: ${FEATURES}"
RUSTFLAGS="-C target-cpu=native" cargo bench --features "${FEATURES}" --no-default-features || true

echo "[perf] Collecting Criterion reports..."
mkdir -p artifacts/criterion
if [ -d target/criterion ]; then
  cp -r target/criterion/* artifacts/criterion/ || true
fi

if [ -f qnet-spec/templates/perf-summary-template.md ]; then
  cp qnet-spec/templates/perf-summary-template.md artifacts/perf-summary.md || true
fi

echo "[perf] Done. See artifacts/ for outputs."
