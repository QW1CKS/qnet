# QNet Helper (stealth-browser) - Installation and Integration

The QNet Helper is a small, local application that runs the `stealth-browser` networking component as a background service. It exposes a local SOCKS5 proxy and an HTTP status/control API that a browser extension or UI can use to control masking, read status, and trigger catalog updates.

## Purpose

- Run the `stealth-browser` Rust binary as a background helper service.
- Expose a stable local API for UI and browser extension integration.
- Manage catalog updates, verification, and atomic persistence.
- Handle system integration tasks (proxy lifecycle, optional hosts file changes).

## Default endpoints

- SOCKS5 proxy: `127.0.0.1:1088`
- Status API: `http://127.0.0.1:8088`
- Update trigger: `POST http://127.0.0.1:8088/update` (can also be GET in dev)

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

## Logging

- Helper writes logs to the repo `logs/` directory when run from source; packaged installers should place logs in OS-appropriate log directories.

## Troubleshooting

- If extension cannot connect to the Helper:
  - Verify Helper is running and listening on the expected ports
  - Check firewall or local policy blocking localhost sockets
  - Check that the extension has native messaging permissions configured


---

For extension developer details, see `qnet-spec/docs/extension.md`.
