# Catalog Publisher Guide

Status: M3 v1 (docs complete) — manual signing now; CI automation planned

This guide explains how to author, sign, and publish the QNet decoy catalog using a dedicated repository (e.g., `qnet-catalogs`). It complements `docs/catalog-schema.md`.

## Repository layout (qnet-catalogs)

- /templates/decoys.yml — human-editable decoys and weights
- /templates/catalog.meta.yml — schema_version, catalog_version, ttl (expires_at), publisher_id, update_urls, seed_fallback_urls
- /keys/publisher.pub — Ed25519 public key (committed); document fingerprints
- /dist/catalog.json — signed catalog (public JSON)
- /dist/catalog.json.sig — detached signature (hex/base64)
- /.github/workflows/build-sign-publish.yml — CI to build, sign, and publish

## Signer inputs and outputs

Inputs:
- YAML templates: `decoys.yml` and `catalog.meta.yml`
- Private key: `CATALOG_PRIVKEY` (Ed25519, hex) stored as repo secret

Outputs:
- `dist/catalog.json` (outer JSON with signature field or detached file)
- `dist/catalog.json.sig` (when using detached signature)

## CI workflow (sketch)

See the detailed YAML snippet in `specs/001-qnet/T6.7-playbook.md` under "catalog publishing automation". Key points:
- Pin action SHAs; minimal permissions
- Build signer (Rust bin) and run with inputs
- Verify signature using `keys/publisher.pub`
- Commit `dist/` and optionally publish a GitHub Release and GitHub Pages mirror

See also template: `qnet-spec/templates/catalog-publish-workflow.yml` for a ready-to-adapt workflow skeleton.

## Manual signing (for now)

Until QNet scales up, manually sign catalogs locally to avoid CI setup overhead:
- Generate/store your private seed (64-char hex) securely (e.g., password manager).
- Set `CATALOG_PRIVKEY` in your shell.
- Run: `cargo run -p catalog-signer -- sign --decoys templates/decoys.yml --meta templates/catalog.meta.yml --out dist/catalog.json --sig dist/catalog.json.sig`
- Verify: `cargo run -p catalog-signer -- verify --catalog dist/catalog.json --sig dist/catalog.json.sig --pubkey-file keys/publisher.pub`
- Commit/upload `dist/` manually to the repo or mirrors.
- When QNet gets big, switch to CI automation with GitHub Secrets for the private key.

## Security practices

- Keep the private key only in CI as an encrypted secret; never commit it
- Rotate keys by shipping overlapping pinned pubkeys in clients and updating `publisher_id`
- Document key fingerprints and rotation schedule in README/UI
- Treat mirrors as untrusted transport; integrity comes from signature over DET-CBOR bytes

## Client update URLs

- Raw: `https://raw.githubusercontent.com/<org>/qnet-catalogs/main/dist/catalog.json`
- Pages: `https://<org>.github.io/qnet-catalogs/catalog.json`
- Optional CDN mirror

## Local testing

- Generate a test keypair (Ed25519) locally and export hex
- Run signer on sample templates in `qnet-spec/templates/`
- Point the app to the cache path with the generated files to test loader/updater logic

Refer to `docs/catalog-schema.md` for field semantics and verification rules.

---

## App bundling guidance (assets + strict verify)

For production builds, bundle a default signed catalog with the app and enforce strict signature verification:

- Place `assets/catalog-default.json` and `assets/catalog-default.json.sig` in the app project (e.g., `apps/stealth-browser/assets/`).
- Embed assets via your app’s bundler (e.g., Tauri asset bundling or include_bytes!/include_str!).
- Ship pinned publisher public keys in the binary. Remove any dev overrides like `STEALTH_CATALOG_ALLOW_UNSIGNED`.
- Loader policy:
	1) Load cached (verify+fresh) → else bundled (verify+fresh)
	2) Reject unsigned or expired catalogs
	3) Persist verified catalogs atomically to cache
- Acceptance:
	- On fresh install with no cache, app starts with bundled catalog verified and status `/status` shows `catalog_source: bundled` and a valid expiry.
	- Tampering with bundled or cached data results in rejection and safe fallback to last-known-good or failure with clear log.

## Manual "Check for updates" contract (UI/backend)

Expose a user-triggered update action to fetch from `update_urls`, verify, and atomically swap if newer:

- Backend command (example): `check_for_updates`
	- Input: none
	- Behavior: attempt fetch from mirrors; verify signature/TTL; replace cache if `catalog_version` increases (or fresher expiry).
	- Output: JSON `{ updated: bool, from: string|null, version: number|null, error: string|null }`
- Status fields (extend `/status` and UI):
	- `last_update_check_ms_ago`, `last_update_ok`, `last_update_error`
	- `catalog_source` becomes `remote` when a newer remote catalog is active

### Decoy catalog artifacts (optional split)

Some deployments may prefer to publish the decoy catalog as a separate artifact alongside the main catalog:

- Add `dist/decoy-catalog.json` and `dist/decoy-catalog.json.sig` to the publisher outputs.
- Reuse the same signing key or define a decoy-specific publisher key (document in clients if different).
- Clients should prefer signed file-based decoy catalogs over environment-provided ones in production.
 - App behavior alignment (M3): the runtime prefers a signed decoy catalog loaded from assets or disk; env-based catalogs are only honored in dev with explicit `STEALTH_DECOY_ALLOW_UNSIGNED=1`.
