# Stealth Browser

This document focuses on M3 (Catalog pipeline) specifics for the Stealth Browser: how the app loads a signed, bundled catalog, verifies it, caches it with TTL, updates from mirrors, and exposes status to the UI. For the high-level playbook, see `specs/001-qnet/T6.7-playbook.md`. For the catalog format and signing, see `docs/catalog-schema.md`.

## M3: Catalog-first pipeline

### What ships in the app (bundled)
- `assets/catalog-default.json` and `assets/catalog-default.json.sig` (detached Ed25519 signature over DET-CBOR of the inner catalog object)
- Pinned Ed25519 publisher public keys compiled into the binary (1–3 allowed for rotation)

### Cache locations (read-write)
- Windows: `%APPDATA%/QNet/catalog.json` (+ `.sig`)
- Linux: `~/.local/share/qnet/catalog.json`
- macOS: `~/Library/Application Support/QNet/catalog.json`

### Loader and verifier (startup flow)
1. Try cached catalog: if present, verify signature using pinned keys; check `expires_at`.
2. If cached is missing/invalid/expired, fall back to bundled catalog (verify again).
3. Activate the freshest valid catalog; retain last-known-good for rollback.
4. Persist updates atomically: write to temp, fsync, rename; keep previous as `.bak`.

### Updater (background or on-demand)
- Sources: `update_urls` array in the active catalog (GitHub Raw/Pages/CDN mirrors).
- Behavior:
  - Fetch `catalog.json` (+ `.sig` if detached) from mirrors with timeout and backoff.
  - Verify signature and `expires_at` freshness; require `catalog_version` > current or fresher `expires_at`.
  - On success, atomically replace cache and emit a status event.
  - On failure, keep current catalog; schedule retry with capped backoff.

### Status API (for UI)
Expose a minimal status struct via Tauri IPC:
```
{
  "catalog": {
    "source": "bundled" | "cached" | "remote",
    "version": <number>,
    "expires_at": "<RFC3339>",
    "publisher_id": "qnet.main.ed25519.2025"
  },
  "bootstrap": {
    "mode": "disabled" | "fallback",
    "last_attempt": "<RFC3339>|null",
    "last_ok": "<RFC3339>|null"
  }
}
```

### Developer templates (used by signer)
- `qnet-spec/templates/decoys.yml` — human-editable entries
- `qnet-spec/templates/catalog.meta.yml` — metadata (`schema_version`, `catalog_version`, `publisher_id`, `update_urls`, optional `seed_fallback_urls`)

### Tests to include in M3 exit
- Unit: signature verification (good/bad), deterministic DET-CBOR bytes, TTL handling (expired vs grace), atomic persist/rollback.
- Integration: updater happy path (newer catalog), tamper rejection, mirror failover, rollback to last-known-good on partial writes.
- E2E (dev): status API shows expected source/version/expiry when switching from bundled → cached → remote.

### Notes
- Mirrors are untrusted transport. Integrity comes solely from signature over DET-CBOR bytes.
- Keep 1–3 pinned keys to enable rotation. Document fingerprints in README and UI.
- Manual vs CI signing: For now, catalogs may be signed locally using `catalog-signer` with `CATALOG_PRIVKEY` set in the shell (store securely). When QNet grows, move to CI automation with GitHub Secrets to avoid local key handling.

## Quick start (dev)

1) Build the app normally; it includes the default catalog assets.
2) Place a test catalog at the cache location to simulate an update (must be valid and newer).
3) Trigger "Check for updates" in the app or via a dev IPC command.
4) Watch status change from `bundled` → `cached` or `remote`.

For catalog authoring and signing, see `docs/catalog-schema.md` and the publisher guide (coming in `docs/catalog-publisher.md`).

## Seedless “online” flip (dev convenience)

For development without bootstrap seeds, the app marks `state: connected` after the first successful SOCKS CONNECT to any target. This lets you validate the end-to-end local path (SOCKS proxy and status server) while staying seedless.

Windows quick test (PowerShell):

1) Start the app in one terminal:
  - Set `STEALTH_STATUS_PORT=18080` and (for examples) `STEALTH_CATALOG_ALLOW_UNSIGNED=1`
  - Run the binary: `P:\GITHUB\qnet\target\debug\stealth-browser.exe`
2) In another terminal, drive one request through the local SOCKS proxy to the status endpoint:
  - Use Windows `curl.exe` (avoid the PowerShell `curl` alias):
    - `%SystemRoot%\System32\curl.exe --socks5-hostname 127.0.0.1:1080 http://127.0.0.1:18080/status`
3) Verify the state:
  - `Invoke-RestMethod http://127.0.0.1:18080/status` → shows `state: connected`, `bootstrap: false`.

Notes:
- This is for dev verification only; production relies on the signed catalog and normal datapath. Remove `STEALTH_CATALOG_ALLOW_UNSIGNED` for strict verification in prod builds.
- If `curl.exe` isn’t on PATH, reference it with the full path shown above.

## Routine Checkup (first-run UX)

On first open, the app runs a short Routine Checkup to ensure it starts from trusted, up-to-date presets without any live seed requirement:

1. Download + verify catalog
  - Fetch `catalog.json` from the `update_urls` mirrors in the bundled catalog; verify Ed25519 signature over DET-CBOR using the pinned publisher pubkey.
  - If newer and valid, atomically persist to cache; otherwise keep bundled.
2. Load decoy catalog (signed)
  - Prefer a signed decoy catalog file shipped with the app or fetched from a trusted mirror; verify signature with the same pinned pubkey unless decoy uses a distinct publisher key.
  - Dev-only fallback: allow an unsigned decoy catalog via env when `STEALTH_DECOY_ALLOW_UNSIGNED=1`.
3. Calibrate decoys
  - Precompute resolver state and ALPN/template hints for plausible egress.
4. Peer discovery (handoff)
  - Initiate discovery/connect (QNet peers) and surface `peers_online` in status. When connected, browsing proceeds via the QNet path.

Status fields exposed during this flow: `checkup_phase`, `catalog_*`, `decoy_count`, `peers_online`.

Masking policy: When a domain matches a `host_pattern` in the decoy catalog, the egress connection is made to the decoy host:port. Outsiders observe the decoy destination (e.g., youtube.com) rather than the original (e.g., google.com). Add catch-alls or specific mappings in the signed decoy catalog to expand coverage.
