# Stealth Browser (Helper service)

This document focuses on M3 (Catalog pipeline) specifics for the Stealth Browser, which is now intended to run as a local Helper service that provides a SOCKS5 proxy and status API. The recommended user-facing deployment pairs a browser extension with this Helper service. For the high-level playbook, see `specs/001-qnet/T6.7-playbook.md`. For the catalog format and signing, see `docs/catalog-schema.md`.

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
 - Manual trigger (dev & UI wiring):
   - Invoke `GET /update` (alias: `GET /check-updates`) on the local status server to perform a one-shot update check.
   - The response includes `{ updated, from, version, error, checked_at_ms }`. The `/status` payload also surfaces the last attempt under `last_update` with `checked_ms_ago`.

### Status API (for UI)
Expose a minimal status struct via the Helper's HTTP status API (used by a Tauri UI or the browser extension):
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

### Decoy catalog (production-ready)
- Edit or create a signed decoy catalog at `qnet-spec/templates/decoy-catalog.json` following the example format.
- Sign it with the `catalog-signer` CLI (or your publisher workflow) using your Ed25519 key.
- At runtime, the app’s Routine Checkup prefers a signed file on disk (or shipped in assets) over any environment-provided values. Env-based decoys are for development only when `STEALTH_DECOY_ALLOW_UNSIGNED=1` is explicitly set.

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
  - Optional: trigger a catalog check with `Invoke-RestMethod http://127.0.0.1:18080/update` and inspect `/status` → `last_update`.

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

### Masking policy defaults (M3)

To provide sensible coverage out of the box while staying plausible, the default decoy catalog includes:

- Specific high-signal pairs:
  - `*.google.com` → `www.youtube.com` (same ecosystem; common cross-traffic)
  - `wikipedia.org` → `www.wikimedia.org` (same foundation)
- Diversified catch-all pool (rotated by weight):
  - `www.cloudflare.com`, `www.microsoft.com`, `www.amazon.com`, `www.apple.com`,
    `www.netflix.com`, `www.reddit.com`, `www.linkedin.com`, `www.yahoo.com`,
    `www.github.com`, `www.stackoverflow.com`

Precedence and selection:
- Longest/most-specific `host_pattern` wins; wildcard `*` applies last.
- Within a pattern, entries are selected by weight (weighted round-robin) to spread load and vary appearance.
- ALPN is set per entry to align with the decoy’s common protocols (e.g., `h2,http/1.1`).

Operations guidance:
- Expand with region-specific decoys as you learn what’s common and reachable in target networks.
- Keep pairs within the same ownership where possible (plausibility for DPI/log analysis).
- Update via the signed catalog workflow; prefer CI-generated artifacts for consistency.

## Edge Gateway (production path)

In Masked mode, the Helper (or a standalone client configured to use the catalog) will call `htx::api::dial()` which shapes the outer TLS to the chosen decoy. A cooperating edge gateway terminates this outer TLS and serves the inner multiplexed stream. The gateway expects an HTTP CONNECT prelude on the inner stream, responds `200 Connection Established`, and then tunnels TCP.

- Gateway binary: `apps/edge-gateway` (env: `BIND`, `HTX_TLS_CERT`, `HTX_TLS_KEY`)
- Library: `htx::api::accept(bind)` under `rustls-config` derives inner keys via TLS exporter (EKM-only).

Local smoke test (dev):
- Generate a self-signed cert and set env as in `qnet-spec/templates/edge-gateway.example.env`.
- Run the gateway, then run the Helper/stealth-browser with `STEALTH_MODE=masked` and a signed decoy catalog.
- Issue a request over SOCKS to any HTTPS target; expect CONNECT success and `state: connected` in `/status`.

### Default ports (Helper)
- SOCKS proxy: `127.0.0.1:1088` (default)
- Status API: `http://127.0.0.1:8088` (default)

The browser extension should point the browser's proxy config to the Helper's SOCKS address and read status via the status API.
