#!/usr/bin/env python3
"""
Lightweight DPI parity helper.

Compares two pcaps (stealth vs baseline) and prints:
 - TLS-like record size distribution CDF buckets
 - JA3-like frequency (approx via port 443 + lengths/records; placeholder)

Dependencies: scapy (pip install scapy)
Usage: python3 templates/dpi-compare.py <stealth.pcap> <baseline.pcap>
"""
import sys
from collections import Counter

try:
    from scapy.all import rdpcap, TCP, UDP
except Exception as e:
    print("scapy is required: pip install scapy")
    sys.exit(1)

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
    for (ba, va), (bb, vb) in zip(ca, cb):
        assert ba == bb
        print(f"  <= {ba:6d}: {va:0.3f} vs {vb:0.3f}  (Δ={abs(va-vb):0.3f})")

def main():
    if len(sys.argv) != 3:
        print("usage: dpi-compare.py <stealth.pcap> <baseline.pcap>")
        sys.exit(2)
    a = load_lengths(sys.argv[1])
    b = load_lengths(sys.argv[2])
    compare(a, b)

if __name__ == "__main__":
    main()
