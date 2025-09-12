#!/usr/bin/env python3
"""
Lightweight DPI parity helper.

Compares two pcaps (stealth vs baseline) and prints:
 - TLS-like record size distribution CDF buckets
 - PASS/FAIL based on max CDF delta vs threshold (default 0.10)

Dependencies: scapy (pip install scapy)
Usage:
    python3 templates/dpi-compare.py <stealth.pcap|glob|dir> <baseline.pcap|glob|dir> [--threshold 0.10]
"""
import sys
import os
import glob
import argparse
from collections import Counter

try:
    from scapy.all import rdpcap, TCP, UDP
except Exception as e:
    print("scapy is required: pip install scapy")
    sys.exit(1)

def resolve_input(spec: str) -> str:
    """Accept a concrete file path, a glob pattern, or a directory.

    - If a directory, use the latest *.pcap* in it.
    - If a glob, expand and pick the latest.
    - If a file path, return as-is.
    """
    candidates = []
    if os.path.isdir(spec):
        candidates = glob.glob(os.path.join(spec, "*.pcap*"))
    else:
        # glob will return [spec] if spec contains no wildcard and exists, else []
        candidates = glob.glob(spec)
        if not candidates and os.path.exists(spec):
            candidates = [spec]
    if not candidates:
        print(f"error: no pcap found for input '{spec}'")
        sys.exit(2)
    latest = max(candidates, key=lambda p: os.path.getmtime(p))
    if len(candidates) > 1:
        print(f"INFO: matched {len(candidates)} files for '{spec}', using latest: {latest}")
    return latest


def load_lengths(pcap_path):
    pkts = rdpcap(pcap_path)
    lengths = []
    for p in pkts:
        if TCP in p and (p[TCP].sport == 443 or p[TCP].dport == 443):
            lengths.append(len(bytes(p)))
        elif UDP in p and (p[UDP].sport == 443 or p[UDP].dport == 443):
            lengths.append(len(bytes(p)))
    return lengths

def cdf_buckets(lengths):
    buckets = [1024, 2048, 4096, 8192, 16384, 65536, 131072]
    total = len(lengths) or 1
    out = []
    for b in buckets:
        out.append((b, sum(1 for x in lengths if x <= b) / total))
    return out

def compare(a, b):
    print(f"Packets: stealth={len(a)} baseline={len(b)}")
    ca = cdf_buckets(a)
    cb = cdf_buckets(b)
    print("CDF (<=bytes : stealth vs baseline)")
    max_delta = 0.0
    for (ba, va), (bb, vb) in zip(ca, cb):
        assert ba == bb
        d = abs(va - vb)
        if d > max_delta:
            max_delta = d
        print(f"  <= {ba:6d}: {va:0.3f} vs {vb:0.3f}  (Δ={d:0.3f})")
    return max_delta

def main():
    parser = argparse.ArgumentParser(description="Compare stealth vs baseline DPI via packet length CDFs on port 443")
    parser.add_argument("stealth", help="stealth pcap path, glob pattern, or directory")
    parser.add_argument("baseline", help="baseline pcap path, glob pattern, or directory")
    parser.add_argument("--threshold", type=float, default=0.10, help="acceptance threshold for max CDF delta (default 0.10)")
    args = parser.parse_args()

    stealth_path = resolve_input(args.stealth)
    baseline_path = resolve_input(args.baseline)
    print(f"Stealth:  {stealth_path}")
    print(f"Baseline: {baseline_path}")

    a = load_lengths(stealth_path)
    b = load_lengths(baseline_path)
    max_delta = compare(a, b)
    status = "PASS" if max_delta <= args.threshold else "FAIL"
    print(f"Result: {status} (max Δ={max_delta:0.3f}, threshold={args.threshold:0.3f})")

if __name__ == "__main__":
    main()
