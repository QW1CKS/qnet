# DPI parity helper — usage and troubleshooting

This folder contains simple helpers to capture port-443 traffic and compare QNet stealth traffic to a Chrome baseline via packet-length CDFs.

Contents:
- dpi-capture.ps1 — Windows PowerShell helper to record 60s of TCP/UDP 443 traffic to artifacts/dpi/
- dpi-capture.sh — Bash helper (Linux/macOS) for similar capture
- dpi-compare.py — Python comparator (supports wildcards and directories) with PASS/FAIL summary

## Prereqs
- Wireshark (includes tshark)
  - Windows: also installs Npcap. Required for capturing on real NICs.
- Python 3.8+ and pip
  - pip install scapy

## Windows quickstart (PowerShell)
1) Open PowerShell as Administrator (Run as administrator).
2) Ensure tshark is available and Npcap is running:
   - If tshark isn’t found, use the full path: "C:\\Program Files\\Wireshark\\tshark.exe -D"
   - Start Npcap:
     - Set-Service -Name npcap -StartupType Automatic
     - Start-Service -Name npcap
     - Get-Service npcap
3) List interfaces (optional):
   - tshark -D
   - Note your active interface index or name (e.g., 3 for Wi‑Fi).
4) Capture 60 seconds each:
   - .\qnet-spec\templates\dpi-capture.ps1 -Label chrome-baseline -DurationSeconds 60 -Interface 3
   - .\qnet-spec\templates\dpi-capture.ps1 -Label qnet-stealth -DurationSeconds 60 -Interface 3
5) Compare:
   - py .\qnet-spec\templates\dpi-compare.py artifacts\dpi\qnet-stealth-*.pcapng artifacts\dpi\chrome-baseline-*.pcapng

The comparator prints CDF buckets and a PASS/FAIL line like:
- Result: PASS (max Δ=0.028, threshold=0.100)

## Linux/macOS notes
- You may need sudo for capture: sudo tshark -D and sudo ./dpi-capture.sh <label>
- Save pcaps under artifacts/dpi/ for consistency.

## Troubleshooting
- tshark not found:
  - Add to PATH: $env:Path += ';C:\\Program Files\\Wireshark' (session only)
  - Or call by full path: & "C:\\Program Files\\Wireshark\\tshark.exe" -D
- Only ETW interface listed on Windows:
  - The Npcap driver is stopped. Run an elevated PowerShell, then:
    - Set-Service -Name npcap -StartupType Automatic
    - Start-Service -Name npcap
- Access denied when starting Npcap:
  - You must use an elevated PowerShell (Run as administrator).
- Empty or tiny pcap:
  - Ensure you browsed some sites during the 60s window.
  - Confirm you selected the active NIC (Wi‑Fi vs Ethernet).
- Comparator wildcard error:
  - The comparator now supports wildcards/dirs and auto-picks the latest; pass the directory or pattern.

## Comparator details
- Input: concrete file, glob pattern (e.g., artifacts/dpi/*.pcapng), or directory (auto-select latest *.pcap*)
- Threshold: --threshold 0.10 (default)
- Output:
  - Packet counts (stealth vs baseline)
  - CDF (<= bytes) for standard buckets
  - PASS/FAIL with max Δ vs threshold

## Artifacts
- Captures are saved to artifacts/dpi/<label>-<timestamp>.pcapng
- Include run metadata (date, host, interface) in commit messages or the playbook when using as evidence.
