#!/usr/bin/env bash
# Simple DPI capture helper (template). Requires tcpdump.
# Usage: ./dpi-capture.sh [outfile-prefix]
# Env:
#   IFACE (default: any)  - network interface to capture
#   DURATION (default: 60) - seconds to capture
#   FILTER (default: 'tcp port 443 or udp port 443') - BPF filter
# Output written under artifacts/dpi/<prefix>-YYYYmmdd-HHMMSS.pcap

set -euo pipefail

IFACE=${IFACE:-any}
DURATION=${DURATION:-60}
FILTER=${FILTER:-'tcp port 443 or udp port 443'}
PREFIX=${1:-qnet-stealth}

ROOT_DIR=$(cd "$(dirname "$0")"/../.. && pwd)
OUT_DIR="$ROOT_DIR/artifacts/dpi"
mkdir -p "$OUT_DIR"
STAMP=$(date +%Y%m%d-%H%M%S)
OUT_FILE="$OUT_DIR/${PREFIX}-${STAMP}.pcap"

echo "[dpi] capturing IFACE=$IFACE DURATION=${DURATION}s FILTER=[$FILTER] -> $OUT_FILE"
command -v tcpdump >/dev/null 2>&1 || { echo "tcpdump is required"; exit 1; }

sudo tcpdump -i "$IFACE" -w "$OUT_FILE" $FILTER -G "$DURATION" -W 1 -nn -U >/dev/null 2>&1 || true

if [ -f "$OUT_FILE" ]; then
  echo "[dpi] capture complete: $OUT_FILE"
  echo "Tip: Compare against a Chrome baseline using templates/dpi-compare.py"
else
  echo "[dpi] no capture file created (permissions or interface issue?)"
  exit 1
fi
