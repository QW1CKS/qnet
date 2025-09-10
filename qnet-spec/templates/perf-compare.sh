#!/usr/bin/env bash
# Compare current Criterion results to a baseline JSON (very simple heuristic)
# Usage: ./perf-compare.sh path/to/baseline_dir path/to/current_dir
# Both directories should contain Criterion benchmark "estimates.json" files in matching structure.
set -euo pipefail

if [ $# -lt 2 ]; then
  echo "usage: $0 <baseline_dir> <current_dir>" >&2
  exit 2
fi

BASE=$1
CURR=$2
THROUGHPUT_DROP=0.10   # 10%
LATENCY_INCREASE=0.15   # 15%

fail=0

compare_file() {
  local base_json=$1
  local curr_json=$2
  # Extract mean estimates (ns) using jq if available; else skip
  if command -v jq >/dev/null 2>&1; then
    base_mean=$(jq -r '.mean.point_estimate // empty' "$base_json" || echo "")
    curr_mean=$(jq -r '.mean.point_estimate // empty' "$curr_json" || echo "")
    if [ -n "$base_mean" ] && [ -n "$curr_mean" ]; then
      # For latency benches: current should be <= base * (1 + LATENCY_INCREASE)
      # If filename contains 'seal_' or 'open_' treat as throughput via inverse of time per byte
      name=$(basename "$(dirname "$curr_json")")
      if [[ "$name" == seal_* || "$name" == open_* ]]; then
        # time ~ 1/throughput, so an increase in time by x% is a drop in throughput by ~x%
        ratio=$(awk -v c=$curr_mean -v b=$base_mean 'BEGIN{print (c/b)}')
        if awk -v r=$ratio -v t=$THROUGHPUT_DROP 'BEGIN{exit r>1+t?0:1}'; then
          echo "[FAIL] Throughput regression in $name: time ratio $ratio (> 1+${THROUGHPUT_DROP})"
          fail=1
        fi
      else
        ratio=$(awk -v c=$curr_mean -v b=$base_mean 'BEGIN{print (c/b)}')
        if awk -v r=$ratio -v t=$LATENCY_INCREASE 'BEGIN{exit r>1+t?0:1}'; then
          echo "[FAIL] Latency regression in $name: time ratio $ratio (> 1+${LATENCY_INCREASE})"
          fail=1
        fi
      fi
    fi
  fi
}

# Walk current dir and compare when baseline exists
while IFS= read -r -d '' f; do
  rel=${f#$CURR/}
  base_f=$BASE/$rel
  if [ -f "$base_f" ]; then
    compare_file "$base_f" "$f"
  fi
done < <(find "$CURR" -name estimates.json -print0)

if [ $fail -ne 0 ]; then
  echo "[perf-compare] Detected regressions beyond thresholds" >&2
  exit 1
else
  echo "[perf-compare] No regressions beyond thresholds"
fi
