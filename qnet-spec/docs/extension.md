# QNet Browser Extension - Developer Guide

This guide explains how the browser extension should integrate with the QNet Helper to provide a user-facing UI for masking, catalog selection, status, and lifecycle control.

## Overview

Recommended production integration:
- Browser extension (UI) communicates with the local Helper via Native Messaging.
- The extension presents the user controls (Start/Stop, Mode select, Catalog chooser) and routes proxy lifecycle commands to the Helper.
- The extension directs the browser to use the Helper's local SOCKS5 proxy (127.0.0.1:1088) while active.

Developer-friendly integration for early testing:
- Use local HTTP or WebSocket control endpoints (Helper must be started with `--enable-dev-api`) on `127.0.0.1:8088`.
- This allows rapid iteration without creating native messaging manifests.

## Native Messaging (production)

1. Manifest file (example, Chrome/Edge):

- Location (Windows): registry key under
  `HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.qnet.helper`
- Manifest content (JSON):
```json
{
  "name": "com.qnet.helper",
  "description": "QNet Helper native messaging",
  "path": "C:\\Program Files\\QNet\\qnet-helper.exe",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://<EXTENSION_ID>/"]
}
```

2. Message format
- Messages are JSON objects prefixed with a 4-byte little-endian length. Use the browser's native messaging API to exchange these.
- Commands: start, stop, status, update, set_catalog, set_mode.

Example command:
```json
{"cmd":"set_mode","mode":"masked"}
```

Response example:
```json
{"status":"ok","mode":"masked","socks_port":1088}
```

## HTTP / WebSocket dev API

- For early development the extension may use HTTP POST/GET to `http://127.0.0.1:8088` endpoints described in `helper.md`.
- Make sure to use localhost-only requests and avoid exposing the API to remote networks.

## UI flow

- On extension install or first-run:
  - Check for a running Helper (try native messaging handshake, fallback to HTTP status check)
  - If Helper not found, prompt user to install the Helper with a provided link and show a "Start Helper" button which opens instructions
- When user clicks "Connect":
  - Send start command to Helper
  - Configure the browser to use 127.0.0.1:1088 as the browser's proxy (use extension proxy APIs when available)
  - Poll `GET /status` for connectivity and catalog state
- When user clicks "Disconnect":
  - Configure browser back to default proxy
  - Send stop command to Helper

## Catalog selection and verification

- The extension should fetch the catalog list from the Helper's `/status` or a catalog list endpoint.
- Allow the user to pick a decoy set and instruct the Helper to switch catalog via `set_catalog`.
- When switching catalogs, the extension should display a brief progress UI while the Helper verifies and activates the new catalog.

## Example extension pseudo-code (background script)

- Handshake box:
  - Try native messaging handshake: send { cmd: 'ping' } and expect { status: 'pong' }
  - If native messaging fails, try HTTP GET /status

- Start logic:
  - send({ cmd: 'start', socks_port: 1088, status_port: 8088 })
  - on success: call browser.proxy.settings.set({value: {mode:'fixed_servers', rules:{singleProxy:{scheme:'socks5', host:'127.0.0.1', port:1088}}}})

- Stop logic:
  - send({ cmd: 'stop' })
  - reset browser.proxy.settings to system default

## Security and privacy

- Never send sensitive data from the extension to remote endpoints unless explicitly authorized by the user.
- Use Native Messaging for command/control in production to limit cross-origin risks.
- Explain to users what "masking" means and that DNS leaks should be mitigated by the extension configuring browser proxy settings.

## Testing

- Manual test flow:
  1. Start Helper manually via `cargo run -p stealth-browser -- --socks-port 1088 --status-port 8088`
  2. Load extension unpacked in Chrome/Edge/Firefox and configure to talk to the helper using HTTP dev API
  3. Click Connect, verify browser uses the local SOCKS5 proxy, and check `GET /status` in the Helper

- Edge cases to test:
  - Helper crash/restart handling
  - Catalog switching during active sessions
  - Permissions prompts for native messaging manifest on install

## Notes for extension developers

- Firefox has a different native messaging host registration path (see MDN docs). Use the official WebExtension guidelines for cross-browser packaging.
- For initial user tests, a simple extension that uses HTTP status endpoints is sufficient. Later replace with native messaging for production.

---

See also: `qnet-spec/docs/helper.md` for Helper API details and `apps/stealth-browser/README.md` for dev run instructions.
