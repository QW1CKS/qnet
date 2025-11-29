# QNet Helper (stealth-browser) - Installation and Integration

The QNet Helper is a small, local application that runs the `stealth-browser` networking component as a background service. It exposes a local SOCKS5 proxy and an HTTP status/control API that a browser extension or UI can use to control masking, read status, and trigger catalog updates.

## Purpose

- Run the `stealth-browser` Rust binary as a background helper service.
- Expose a stable local API for UI and browser extension integration.
- Manage catalog updates, verification, and atomic persistence.
- Handle system integration tasks (proxy lifecycle, optional hosts file changes).

## Default Endpoints

Local-only (binds 127.0.0.1) service surface; never expose externally:

| Path / Service | Method | Purpose |
|----------------|--------|---------|
| `127.0.0.1:1088` | SOCKS5 | Browser proxy (masked / direct modes) |
| `/` | GET | Human HTML status page (auto-polls `/status`) |
| `/status` | GET | Canonical JSON snapshot (state, mode, masking + catalog) |
| `/status.txt` | GET | Plain text summary (greppable) |
| `/ready` | GET | Lightweight readiness probe (always `ok` if thread alive) |
| `/metrics` | GET | Minimal internal counters (connections) |
| `/ping` | GET | Health ping `{ ok, ts }` |
| `/config` | GET | Sanitized runtime configuration (ports, mode) |
| `/update` | GET/POST | (Dev) Trigger catalog update check |

All HTTP endpoints served from `http://127.0.0.1:8088` by default.

### `/status` JSON Fields

Additive, presence-based (omit if unknown):

`state`, `mode`, `config_mode`, `socks_addr`, `decoy_count`, `current_target`, `current_decoy`, `catalog_version`, `catalog_expires_at`, `catalog_source`, `peers_online`, `checkup_phase`, `last_checked_ms_ago`, `masked_attempts`, `masked_successes`, `masked_failures`, `last_masked_error`, `seed_url`, `last_update.{updated,from,version,error,checked_ms_ago}`.

Notes:
- Query parameters (`/status?ts=...`) are accepted (cache-busting) and ignored server side.
- Prefer JSON for automation; `/status.txt` is intentionally minimal.

## Installation

### Windows
- Deliver as an MSI/EXE installer that:
  - Installs the Helper binary to `%ProgramFiles%\QNet\` or user-local app dir
  - Optionally registers a Windows Service for auto-start (installer prompts for admin on first run)

### macOS / Linux
- Provide a platform-specific package (PKG, DEB, RPM) or a simple tarball with a systemd unit for Linux.

## Integration primitives

### 1) Native Messaging (recommended for production)

- Extension registers a native messaging host manifest pointing to the Helper executable.
- Communication uses JSON messages over stdin/stdout (Native Messaging protocol).
- Use the native messaging channel to request: start, stop, status, update, and to stream logs.

Example request:
```json
{"cmd":"start","socks_port":1088,"status_port":8088}
```

Example response:
```json
{"status":"ok","pid":12345}
```

### 2) Local WebSocket / HTTP (development-friendly)

- Helper can optionally open a local WebSocket or HTTP control endpoint on `127.0.0.1` to receive control requests from the extension.
- This is easier for early development but requires careful CORS/localhost hardening before production.

## Security considerations

- Only listen on `127.0.0.1` by default.
- Use the browser's native messaging when possible to avoid arbitrary local requests.
- When performing hosts file edits, require explicit user consent and explain the change.
- Validate and rate-limit IPC commands to avoid abuse from other local processes.

## Commands / API (HTTP)

- `GET /status` — returns JSON with current catalog, mode, decoy_count, and last_update
- `POST /start` — start the proxy (body: { socks_port, status_port, mode })
- `POST /stop` — stop the proxy
- `POST /update` — trigger a catalog update check, returns `{updated, error?}`
- `GET /logs?tail=100` — tail recent logs

## Dev flags

- `HTX_INNER_PLAINTEXT=1` — use plaintext inner mux for debugging
- `HTX_EKM_EXPORTER_NO_CTX=1` — simplify EKM exporter context for compatibility tests
- `STEALTH_DECOY_ALLOW_UNSIGNED=1` — allow unsigned dev catalogs (dev only)

## Example: start helper manually (dev)

```powershell
# From repo root
cargo run -p stealth-browser -- --socks-port 1088 --status-port 8088
```

## Quick Masked Connection Test (Windows / PowerShell 7+)

End-to-end smoke test to verify a masked tunnel is operational.

### Prerequisites

- PowerShell 7 (`pwsh`) — verify: `$PSVersionTable.PSVersion.Major -ge 7`
- Rust toolchain (1.70+) with build dependencies
- Signed decoy catalog available (env `STEALTH_DECOY_CATALOG_JSON`) OR dev catalog file + `--allow-unsigned-decoy` (development only)
- `curl` executable in PATH

### One-liner (ad hoc)

```powershell
pwsh -NoProfile -Command "cargo build -q -p stealth-browser; Start-Process -PassThru -WindowStyle Hidden target\\debug\\stealth-browser.exe --mode masked; for($i=0;$i -lt 40;$i++){ try { $s=Invoke-RestMethod http://127.0.0.1:8088/status; if($s.state -eq 'connected'){break}; Start-Sleep 1 } catch {}; }; curl.exe -I https://www.wikipedia.org --socks5-hostname 127.0.0.1:1088"
```

### Scripted (create `scripts/test-masked-connection.ps1`)

```powershell
param(
  [string]$Origin = 'https://www.wikipedia.org',
  [int]$TimeoutSeconds = 60
)
Write-Host "[qnet] launching stealth-browser (masked)" -ForegroundColor Cyan
Start-Process -WindowStyle Hidden -FilePath "$PSScriptRoot/../target/debug/stealth-browser.exe" -ArgumentList '--mode','masked' | Out-Null
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
do {
  try { $s = Invoke-RestMethod http://127.0.0.1:8088/status -TimeoutSec 2 } catch { Start-Sleep 1; continue }
  if ($s.state -eq 'connected') { break }
  Start-Sleep 1
} while ((Get-Date) -lt $deadline)
if ($s.state -ne 'connected') { Write-Warning "Did not reach connected state (state=$($s.state))" }
Write-Host "[qnet] curl HEAD $Origin via SOCKS5" -ForegroundColor Cyan
curl.exe -I $Origin --socks5-hostname 127.0.0.1:1088
Write-Host "[qnet] status snapshot:" -ForegroundColor Cyan
Invoke-RestMethod http://127.0.0.1:8088/status | ConvertTo-Json -Depth 4
```

### Expected Outcome

1. `/status` transitions to `connected` (or `calibrating` then `connected`).
2. `curl` shows a valid HTTP response from the origin.
3. `masked_attempts` and `masked_successes` increment; `current_decoy` populated.

If failure:
- Check `logs/stealth-browser*.log`
- Inspect `/status.txt` for concise summary
- Ensure catalog loaded (`catalog_version` present)

### Security Warning

Avoid `--allow-unsigned-decoy` in production: unsigned catalogs bypass signature & expiry verification.

## Peer Discovery and Registration

### Relay-Only Mode (Default for Relay Nodes)

When Helper runs in relay-only mode (`--relay-only` or `helper_mode: RelayOnly`):

1. **Startup**: Query operator node at `http://<operator-ip>:8088/api/relays/by-country`
2. **Heartbeat**: POST to `http://<operator-ip>:8088/api/relay/register` every 30 seconds
3. **Payload Example**:
   ```json
   {
     "peer_id": "12D3KooW...",
     "addrs": ["/ip4/192.168.1.100/tcp/4001"],
     "country": "US",
     "capabilities": ["relay"],
     "last_seen": 1732900000,
     "first_seen": 1732900000
   }
   ```
4. **Pruning**: If no heartbeat received for 2 minutes, peer is removed from directory

### Bootstrap Mode (Operator Nodes Only)

When Helper runs in bootstrap mode (`--bootstrap` or `helper_mode: Bootstrap`):

1. **Directory Service**: Host HTTP endpoints:
   - `POST /api/relay/register` - Accept relay registrations
   - `GET /api/relays/by-country` - Return peer list grouped by country
2. **Storage**: In-memory `HashMap<PeerId, RelayInfo>` with last_seen timestamps
3. **Pruning**: Background task removes stale peers (no heartbeat for 2+ minutes) every 60 seconds
4. **Response Format**:
   ```json
   {
     "US": [{"peer_id": "...", "addrs": [...], "capabilities": [...]}],
     "FR": [{"peer_id": "...", "addrs": [...], "capabilities": [...]}]
   }
   ```

### Client Mode (Default for Users)

When Helper runs in client mode (default, no flags):

1. **Startup**: Query operator directory for relay peer list
2. **Dial**: Attempt connection to relay peers from directory
3. **Fallback**: If directory query fails, dial hardcoded operator nodes directly
4. **No Registration**: Client nodes do not register with directory (privacy)

## Logging

- Helper writes logs to the repo `logs/` directory when run from source; packaged installers should place logs in OS-appropriate log directories.

## Troubleshooting

- If extension cannot connect to the Helper:
  - Verify Helper is running and listening on the expected ports
  - Check firewall or local policy blocking localhost sockets
  - Check that the extension has native messaging permissions configured


---

For extension developer details, see `qnet-spec/docs/extension.md`.
