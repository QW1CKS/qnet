# Physical Testing Playbook

This playbook covers hands-on validation of QNet in real networks. It complements automated tests and focuses on repeatable, Windows-friendly procedures using the recommended Browser Extension + Helper model.

- Default helper endpoints: SOCKS5 127.0.0.1:1088, Status API http://127.0.0.1:8088
- Edge Gateway default: 0.0.0.0:4443 (TLS)
- Key docs: see `qnet-spec/docs/helper.md` and `qnet-spec/docs/extension.md`

## Objectives
- Verify basic connectivity and routing using two or more machines.
- Validate stealth characteristics (HTTPS mimicry) with packet captures.
- Measure latency/throughput and stability under load.
- Exercise failure modes (network disruptions) and recovery.
- Validate decoy catalog use and catalog update path.

## Topologies
- Single laptop (dev smoke test): Helper + Edge Gateway locally, browser via extension → SOCKS5 1088.
- Two-node LAN: Client (browser+helper) ↔ Edge Gateway on another machine.
- Three-node with decoy: Client → Decoy host (TLS) → Edge Gateway (tunnel) → test server.
- WAN variant: Same as above but across the Internet/VPN.

## Prerequisites
- Windows 10/11 (PowerShell 7), or Linux/macOS equivalents where noted.
- Rust toolchain for building binaries (if not using prebuilt).
- Browser supporting WebExtensions (Firefox/Chrome dev mode) for extension tests.
- Wireshark for packet capture; curl or httpie; iperf3 for throughput.
- Admin rights to install a local TLS cert for edge (optional for dev).

## Environment variables (dev)
- STEALTH_MODE=1 (enables framing/jitter/padding)
- STEALTH_DECOY_ALLOW_UNSIGNED=1 (dev only)
- STEALTH_DECOY_CATALOG_JSON=path/to/catalog.json
- HTX_INSECURE_NO_VERIFY=1 (dev only)
- PREFER_QUIC=0|1 (toggle if applicable)

## Safety and hygiene
- Do not run dev-insecure flags in production.
- Prefer an isolated test subnet or VLAN when experimenting with DPI.
- Capture only traffic relevant to the test; avoid personal data.

## Procedures

### 1) Network sanity check
- Connect two machines to the same subnet. Assign static IPs (e.g., 192.168.1.10/24 and 192.168.1.20/24). Verify ICMP ping both ways.
- Record baseline ping min/avg/max for comparison.

### 2) Edge Gateway bring-up
- On server machine: start edge-gateway listening on 0.0.0.0:4443 with logs to `logs/edge-gateway-*.log`.
- Verify port open with `Test-NetConnection -ComputerName <server-ip> -Port 4443` (Windows) or `nc -zv`.

### 3) Helper + Extension (client)
- Start `stealth-browser` helper with defaults (SOCKS5 1088, status 8088). Confirm status endpoint returns JSON (health, catalog version, connected edge, mode).
- Install/load the browser extension (dev mode). Configure it to use SOCKS5 127.0.0.1:1088.
- Browse a known reachable site; verify requests flow via the helper (status counters increment).

### 4) HTTP echo via tunnel
- On server machine: `python -m http.server 8080` or similar.
- From client browser: navigate to `http://<server-ip>:8080/`. If policy requires tunnel through edge, set the helper to route via edge gateway.
- Expected: page loads; helper logs a CONNECT/STREAM; edge logs a corresponding flow.

### 5) Stealth capture
- Start Wireshark on client; capture on interface used.
- With STEALTH_MODE=1, initiate browsing through the helper.
- Verify:
  - Outer flow looks like TLS (SNI/ALPN shape if applicable; no QNet markers).
  - Record sizing/jitter resembles baseline HTTPS of a popular site.
  - No plaintext inner frames visible.

### 6) Performance quick check
- Run iperf3 server on the far end (or HTTP large download).
- From client, fetch a 50–100MB file through the tunnel.
- Note throughput and CPU. Compare to direct path. Target: added latency <50ms local; throughput >100 Mbps on LAN.

### 7) Failure and recovery
- While streaming data, disconnect the client from the network for ~5s; reconnect.
- Expected: transport re-establishes (if supported) or session resumes cleanly; minimal data loss; logs show graceful handling.

### 8) Decoy routing (advanced)
- Configure a decoy catalog with one decoy endpoint and enable decoy-first in the helper.
- Confirm ISP-visible logs reference the decoy IP/domain; inner destination remains hidden.

## Data capture template
- Date/Time, Test ID, Topology, OS/Versions (Rust, Browser), Helper version/hash, Edge version/hash
- Env flags used
- Expected vs Actual, Pass/Fail
- Attach: pcap, helper/edge logs (redact secrets), screenshots

## Troubleshooting
- CONNECT prelude not accepted: ensure status API shows healthy, verify ALPN/template settings, set HTX_INNER_PLAINTEXT=1 for local diag only and retry.
- TLS errors: check certs under `certs/`, set HTX_INSECURE_NO_VERIFY=1 only for dev.
- No traffic: confirm extension proxy settings and that SOCKS5 port 1088 is listening.
- Slow throughput: disable antivirus inspection temporarily, verify no duplicate captures, check CPU saturation.

## Acceptance checklist
- [ ] Basic client ↔ edge flow established
- [ ] Browser moves traffic through helper (counters increase)
- [ ] Wireshark shows TLS-like outer traffic; no QNet markers
- [ ] Latency/throughput within stated targets on LAN
- [ ] Recovery after brief network loss
- [ ] Decoy routing produces expected observables

## Appendix: Commands (Windows PowerShell)
- Port check: `Test-NetConnection -ComputerName <ip> -Port 4443`
- Process listening: `Get-NetTCPConnection -LocalPort 1088,8088 | Select-Object LocalAddress,LocalPort,State`
- Curl over SOCKS5: `curl.exe --socks5 127.0.0.1:1088 http://example.com`
- Wireshark capture filter suggestion: `tcp port 4443`

