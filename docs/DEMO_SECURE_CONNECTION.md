# Demo: Secure Connection with QNet

This guide demonstrates a full end-to-end secure connection using QNet, including bootstrap discovery, TLS handshake, decoy routing, and DPI evasion verification.

## Prerequisites

- **Rust and Cargo**: Install from [rustup.rs](https://rustup.rs). Ensure `cargo` is on PATH.
- **Wireshark/Npcap**: Install Wireshark from [wireshark.org](https://www.wireshark.org/download.html) (includes Npcap driver). Ensure `tshark.exe` is on PATH or note its full path.
- **Python**: For DPI comparison (optional). Install Python 3.x and `scapy` via `pip install scapy`.
- **Bootstrap Seeds**: Publicly reachable HTTPS endpoints returning 200 on `/health`. Use Cloudflare Quick Tunnels (e.g., `cloudflared tunnel --url http://localhost:8080`) or provide your own.
- **Npcap Service**: Ensure the `npcap` service is running (check with `Get-Service npcap` in PowerShell).

## Quick Demo Steps (Windows PowerShell)

1. **Set up bootstrap seeds** (replace with your tunnel URLs):
   ```powershell
   $seeds = "https://your-tunnel1.trycloudflare.com https://your-tunnel2.trycloudflare.com"
   ```

2. **Validate seeds**:
   ```powershell
   foreach ($u in $seeds -split ' ') { Invoke-WebRequest $u -UseBasicParsing -TimeoutSec 10; Write-Host "$u -> $($_.StatusCode)" }
   ```

3. **Run bootstrap check**:
   ```powershell
   $env:STEALTH_BOOTSTRAP_CATALOG_JSON = '{"catalog":{"version":1,"updated_at":1726128000,"entries":[{"url":"https://your-tunnel1.trycloudflare.com"},{"url":"https://your-tunnel2.trycloudflare.com"}]}}'
   $env:STEALTH_BOOTSTRAP_ALLOW_UNSIGNED = '1'
   cargo run -q -p htx --example bootstrap_check
   ```

4. **Run secure dial demo** (with decoy routing):
   ```powershell
   $env:STEALTH_DECOY_CATALOG_JSON = '{"catalog":{"version":1,"updated_at":1726128000,"entries":[{"host_pattern":"*","decoy_host":"www.cloudflare.com","port":443,"alpn":["h2","http/1.1"],"weight":1}]}}'
   $env:STEALTH_DECOY_ALLOW_UNSIGNED = '1'
   $env:STEALTH_LOG_DECOY_ONLY = '1'
   cargo run -q -p htx --features rustls-config --example dial_tls_demo -- https://www.wikipedia.org
   ```

5. **Capture and compare DPI** (optional, requires Wireshark):
   - Find interface index: `tshark -D`
   - Capture during dial:
     ```powershell
     .\qnet-spec\templates\dpi-capture.ps1 -Label qnet-stealth -DurationSeconds 60 -Interface 3 -TsharkExe 'C:\Program Files\Wireshark\tshark.exe'
     ```
   - Compare to Chrome baseline:
     ```powershell
     py .\qnet-spec\templates\dpi-compare.py artifacts\dpi\qnet-stealth-*.pcapng artifacts\dpi\chrome-baseline-*.pcapng
     ```
     - PASS if max Δ < 0.1 (indicates traffic looks like normal TLS).

## One-Click Demo Script

Use the provided PowerShell script for automation:

```powershell
.\scripts\demo-secure-connection.ps1 `
  -WithDecoy `
  -Origin https://www.wikipedia.org `
  -CaptureSeconds 60 `
  -Interface 3 `
  -SeedsList "https://your-tunnel1.trycloudflare.com https://your-tunnel2.trycloudflare.com"
```

This script handles env setup, validation, bootstrap, decoy, capture, and dial in sequence.

## Expected Output

- Bootstrap: "bootstrap: ok -> https://..."
- Dial: "connected and opened an HTX secure stream"
- Capture: "Saved: artifacts\dpi\qnet-stealth-...pcapng"
- Compare: "Result: PASS (max Δ=..., threshold=0.100)"

## Troubleshooting

- **Bootstrap fails**: Ensure seeds return 200 on `/health` within 30s. Check firewall/proxy.
- **Dial fails**: Verify Rust features (`--features rustls-config`). Check decoy catalog format.
- **Capture fails**: Ensure Npcap is running (`Start-Service npcap`). Use correct interface index from `tshark -D`.
- **Comparator fails**: Install `scapy` (`pip install scapy`). Ensure pcaps exist in `artifacts/dpi/`.
- **PowerShell errors**: Use PS 5.1+; avoid positional binding issues by specifying parameter names.
- **PATH issues**: Explicitly pass `-TsharkExe` if `tshark` not on PATH.

## What This Demonstrates

- **Bootstrap**: Discovers healthy seeds for initial trust.
- **Secure Connection**: Performs real TLS handshake, derives inner keys via EKM, opens HTX stream.
- **Decoy Routing**: Routes outer TLS to decoy host to evade censorship.
- **DPI Evasion**: Verifies traffic shape matches normal TLS (CDF comparison).

For more details, see [ARCHITECTURE.md](ARCHITECTURE.md).</content>
<parameter name="filePath">p:\GITHUB\qnet\docs\DEMO_SECURE_CONNECTION.md