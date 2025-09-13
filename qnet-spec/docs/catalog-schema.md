# QNet Catalog: Schema, Signing, and Update Model

Status: Draft (adopt for M3); Aligns with existing DET-CBOR + Ed25519 signatures in `htx::decoy` and `htx::bootstrap`.

---

## Overview

QNet clients use a signed “catalog” as the primary configuration for plausible decoy routing and related metadata. The catalog is:
- Bundled with the app (default, offline-safe)
- Cacheable on disk with a TTL
- Updatable from one or more public mirrors (GitHub/CDN)
- Verifiable via a pinned publisher public key
- With seeds kept as an optional resilience/emergency fallback

This document defines the JSON structure presented to users and mirrors, the canonical form that is signed (DET-CBOR), and operational guidance for keys, updates, and rotation.

---

## File formats

- Public representation (what we host/ship):
  - `catalog.json`: JSON object containing the inner catalog fields
  - `catalog.json.sig` or `signature_hex` field: Ed25519 signature (detached) over DET-CBOR of the inner catalog object

- Canonical signed representation:
  - `DET-CBOR(catalog)` (deterministic CBOR encoding of the JSON `catalog` object)
  - `signature = Ed25519.sign(sha512(DET-CBOR(catalog)))` (or direct Ed25519 over bytes if preferred)

Note: We keep JSON for human readability and transport; verification always converts the JSON `catalog` object to DET-CBOR bytes before signature verification.

---

## JSON schema (outer file)

Top-level object fields:
- `schema_version` (number): Schema version for compatibility (start at 1)
- `catalog_version` (number): Monotonic version to compare freshness
- `generated_at` (RFC3339 string): Timestamp the catalog was generated
- `expires_at` (RFC3339 string): TTL boundary for freshness
- `publisher_id` (string): Publisher key identifier (e.g., `qnet.main.ed25519.2025`)
- `update_urls` (array<string>): Mirrors to fetch newer catalogs from
- `seed_fallback_urls` (array<string>, optional): Bootstrap seed endpoints as a resilience fallback
- `entries` (array<object>): Decoy entries (see below)
- `signature_hex` (string): hex-encoded Ed25519 signature over DET-CBOR of the object above, excluding `signature_hex`

Decoy entry fields:
- `id` (string): Stable identifier
- `host` (string): Decoy hostname (SNI/front domain)
- `ports` (array<number>): e.g., `[443]`
- `protocols` (array<string>): e.g., `["tcp","quic"]`
- `alpn` (array<string>): e.g., `["h2","h3"]`
- `region` (array<string>, optional): e.g., `["global","cdn"]`
- `weight` (number, optional): Selection weight (default 1)
- `health_path` (string, optional): Path for health check (e.g., `/` or `/health`)
- `tls_profile` (string, optional): Template label (e.g., `chrome-like-2025q3`)
- `quic_hints` (object, optional): Tuning hints (e.g., `{"cid_len":8}`)

Example (trimmed):
```
{
  "schema_version": 1,
  "catalog_version": 42,
  "generated_at": "2025-09-12T12:00:00Z",
  "expires_at": "2025-09-19T12:00:00Z",
  "publisher_id": "qnet.main.ed25519.2025",
  "update_urls": [
    "https://raw.githubusercontent.com/org/qnet-catalogs/main/catalog.json",
    "https://cdn.example.org/qnet/catalog.json"
  ],
  "seed_fallback_urls": ["https://seed1.example.org/bootstrap.json"],
  "entries": [
    { "id": "cdn-01", "host": "www.example-cdn.com", "ports": [443], "protocols": ["tcp","quic"], "alpn": ["h2","h3"], "region": ["global","cdn"], "weight": 10, "health_path": "/", "tls_profile": "chrome-like-2025q3" }
  ],
  "signature_hex": "ed25519_signature_hex_over_DET_CBOR"
}
```

---

## Verification procedure (client)

1) Parse JSON, extract `signature_hex`, and build the inner `catalog` object by removing `signature_hex`.
2) Canonicalize: encode the `catalog` object using DET-CBOR (deterministic CBOR).
3) Verify: check `Ed25519.verify(signature_hex, DET-CBOR(catalog), pinned_pubkeys[])`.
4) Check freshness: ensure `now < expires_at` (or allow grace with warnings).
5) Accept on success; otherwise fall back to cached or bundled catalog, or seeds if enabled.

Pinned keys: embed 1–3 Ed25519 public keys in code; allow key rotation by shipping overlapping keys for at least one release.

---

## Update model

- The app periodically (or on user request) fetches `catalog.json` and `catalog.json.sig` (or `signature_hex` inlined) from `update_urls`.
- It verifies signature and freshness; if newer/fresher than the cached copy, it persists atomically and swaps the active catalog.
- If a mirror is unreachable or blocked, other URLs are tried. If all fail, the cached or bundled catalog is used until expiry.
- Seeds (`seed_fallback_urls`) remain optional and are used when no valid catalog is available and mirrors are inaccessible.

---

## Bundling and cache locations

- Bundled defaults (read-only): included in the app at build time (e.g., `assets/catalog-default.json` + `.sig`)
- Cache (read-write):
  - Windows: `%APPDATA%/QNet/catalog.json` (+ `.sig`)
  - Linux: `~/.local/share/qnet/catalog.json`
  - macOS: `~/Library/Application Support/QNet/catalog.json`

Persist with atomic replace (write temp → fsync → rename) and keep the previous copy for rollback.

---

## Security notes

- Never include secrets in the catalog. Treat it as integrity-critical, not confidential.
- Verification must always use the canonical DET-CBOR bytes.
- Enforce TTL; warn users when operating on an expired catalog and attempt updates.
- Document key rotation; publish pubkey fingerprints in README and app UI.

---

## Relationship to existing modules

- Aligns with `htx::decoy` signed catalog (Ed25519 over deterministic CBOR) and `htx::bootstrap` signed seeds.
- Migration: make catalog-first the default; invoke seeds only as a fallback when no valid catalog can be loaded/updated.
