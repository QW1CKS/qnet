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

### Helper Modes (Task 2.1.11.3)

QNet Helper supports five operational modes, configurable via CLI (`--helper-mode <MODE>`) or environment variable (`STEALTH_MODE`):

#### 1. Client Mode (default)
**Usage**: End-user devices (laptops, desktops)
```bash
stealth-browser --helper-mode client
# or
STEALTH_MODE=client stealth-browser
```

**Behavior**:
- Query operator directory for relay peer list on startup
- Dial discovered relay peers for circuit building
- **Does NOT register** with directory (privacy: no operator visibility)
- **Does NOT run** directory service endpoints
- **Does NOT support** exit node functionality

**Feature Summary**: Query directory, no registration, no exit

---

#### 2. Relay Mode
**Usage**: Trusted relay nodes (forwarding encrypted packets only)
```bash
stealth-browser --helper-mode relay
# or
STEALTH_MODE=relay stealth-browser
```

**Behavior**:
- Query operator directory on startup (discover other relays)
- **Registers with directory** via heartbeat (POST `/api/relay/register` every 30s)
- Relay encrypted traffic for other peers
- **Does NOT run** directory service endpoints
- **Does NOT support** exit node functionality (safe, no legal liability)

**Heartbeat Payload**:
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

**Feature Summary**: Register with directory, relay traffic, no exit

---

#### 3. Bootstrap Mode
**Usage**: Operator-run directory servers (6 global droplets)
```bash
stealth-browser --helper-mode bootstrap
# or
STEALTH_MODE=bootstrap stealth-browser
```

**Behavior**:
- **Runs directory service** HTTP endpoints:
  - `POST /api/relay/register` - Accept relay registrations
  - `GET /api/relays/by-country?country=US` - Return peer list (optionally filtered by country)
  - `GET /api/relays/prune` - Manual pruning trigger (dev only)
- Relay encrypted traffic for other peers
- **Does NOT send heartbeats** (bootstrap nodes don't register with themselves)
- **Does NOT query directory** on startup (self-contained)
- **Does NOT support** exit node functionality

**Directory Storage**: In-memory `HashMap<PeerId, RelayInfo>` with automatic pruning (stale entries removed after 2 minutes without heartbeat)

**Pruning**: Background task runs every 60 seconds, removes peers with `last_seen` > 120 seconds ago

**Response Format** (`GET /api/relays/by-country`):
```json
{
  "US": [{"peer_id": "...", "addrs": [...], "capabilities": ["relay"], "last_seen": 1732900000}],
  "FR": [{"peer_id": "...", "addrs": [...], "capabilities": ["relay"], "last_seen": 1732900050}]
}
```

**Feature Summary**: Run directory service, relay traffic, no exit

---

#### 4. Exit Mode
**Usage**: Dedicated exit nodes (forwarding to public internet)
```bash
stealth-browser --helper-mode exit
# or
STEALTH_MODE=exit stealth-browser
```

**Behavior**:
- Query operator directory on startup
- **Registers with directory** via heartbeat
- Relay encrypted traffic for other peers
- **Exit to internet** enabled (handle HTTP/HTTPS CONNECT requests)
- **Does NOT run** directory service endpoints

**Exit Node Features** (Task 2.1.11.2):
- TLS Passthrough (no MITM, end-to-end encryption preserved)
- Port filtering: Allow 80/443, block 25/110/143 (SMTP/POP3/IMAP)
- SSRF prevention: Block localhost, 127.0.0.1, private IP ranges (RFC 1918)
- Rate limiting: 20 connections/client, 100 requests/min/client
- Logging: Sanitized (destination host/port only, no PII, 7-day retention)

**Legal Considerations**:
- DMCA §512(a) safe harbor (US): Transitory network communications exemption
- E-Commerce Directive Article 12 (EU): Mere conduit safe harbor
- Operator responsible for traffic from exit node IP address
- Abuse contact email required (`EXIT_ABUSE_EMAIL` env var)

**⚠️ WARNING**: Exit node mode enables internet forwarding. Legal liability applies. Your IP will be visible to destination websites.

**Feature Summary**: Relay + exit to internet, no directory service

---

#### 5. Super Mode
**Usage**: Operator-run super peers (all features enabled)
```bash
stealth-browser --helper-mode super
# or
STEALTH_MODE=super stealth-browser
```

**Behavior**:
- **Runs directory service** (bootstrap endpoints)
- **Registers with directory** via heartbeat
- Relay encrypted traffic for other peers
- **Exit to internet** enabled (full exit node functionality)

**Use Case**: Operator droplets serving as combined bootstrap + relay + exit nodes (6-droplet global infrastructure)

**Feature Summary**: All features (bootstrap + relay + exit)

---

### Mode Comparison Table

| Feature | Client | Relay | Bootstrap | Exit | Super |
|---------|--------|-------|-----------|------|-------|
| Query directory on startup | ✅ | ✅ | ❌ | ✅ | ✅ |
| Register with directory (heartbeat) | ❌ | ✅ | ❌ | ✅ | ✅ |
| Run directory service | ❌ | ❌ | ✅ | ❌ | ✅ |
| Relay encrypted traffic | ✅ | ✅ | ✅ | ✅ | ✅ |
| Exit to internet | ❌ | ❌ | ❌ | ✅ | ✅ |
| Legal liability | No | No | No | **Yes** | **Yes** |
| Typical deployment | User devices | Trusted relays | Operator droplets | Exit relays | Operator droplets |

---

### Connection Behavior and Limitations

#### Current Behavior (Dec 2025)

**libp2p Connection Management**:
- Helper uses libp2p (TCP + Noise + yamux) for peer-to-peer connections
- Connections are established to bootstrap nodes on startup
- Default idle timeout: ~60 seconds (libp2p KeepAliveTimeout)

**Known Limitation - Idle Disconnects**:
The current implementation may disconnect from peers after ~1 minute of idle time due to libp2p's default keepalive behavior. This manifests as:
- Status shows `peers_online: 0` after period of inactivity
- Logs show: `Disconnected from peer ... (cause: Some(KeepAliveTimeout))`
- SOCKS5 traffic still works via direct HTX if connected

**Planned Improvements** (Task 2.1.12):
1. **Keepalive Pings**: Configure libp2p ping protocol with 30-second intervals
2. **Automatic Reconnection**: Reconnect loop to re-dial bootstrap nodes when disconnected
3. **Connection Health Indicators**: New `/status` fields for mesh health

#### Troubleshooting Connection Issues

If `/status` shows `peers_online: 0`:
1. **Check bootstrap node availability**: `curl http://104.248.22.27:8088/ping`
2. **Check logs**: Look for `KeepAliveTimeout` or connection errors in `logs/stealth-browser*.log`
3. **Force reconnection**: Restart the Helper (`Ctrl+C` then re-run)
4. **Verify network**: Ensure outbound TCP port 4001 is not blocked

---

### Environment Variables

- `STEALTH_MODE` - Helper mode (overrides CLI flag): `client`, `relay`, `bootstrap`, `exit`, `super`
- `STEALTH_SOCKS_PORT` - SOCKS5 port (default: 1088)
- `STEALTH_STATUS_PORT` - Status API port (default: 8088)
- `EXIT_ABUSE_EMAIL` - Abuse contact email (required for exit/super modes)
- `EXIT_MAX_CONNECTIONS` - Max concurrent exit connections (default: 1000)
- `EXIT_ALLOWED_PORTS` - Comma-separated allowed ports (default: 80,443)

---

### Legacy Aliases (Backward Compatibility)

For backward compatibility, the following CLI flags still work:

- `--relay-only` → `--helper-mode relay`
- `--exit-node` → `--helper-mode exit`
- `--bootstrap` → `--helper-mode bootstrap`

Environment variable `QNET_MODE` is deprecated; use `STEALTH_MODE` instead.

---

## Logging

- Helper writes logs to the repo `logs/` directory when run from source; packaged installers should place logs in OS-appropriate log directories.

## Troubleshooting

- If extension cannot connect to the Helper:
  - Verify Helper is running and listening on the expected ports
  - Check firewall or local policy blocking localhost sockets
  - Check that the extension has native messaging permissions configured


---

For extension developer details, see `qnet-spec/docs/extension.md`.
