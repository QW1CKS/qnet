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

## Quick start (dev)

1) Build the app normally; it includes the default catalog assets.
2) Place a test catalog at the cache location to simulate an update (must be valid and newer).
3) Trigger "Check for updates" in the app or via a dev IPC command.
4) Watch status change from `bundled` → `cached` or `remote`.

For catalog authoring and signing, see `docs/catalog-schema.md` and the publisher guide (coming in `docs/catalog-publisher.md`).
