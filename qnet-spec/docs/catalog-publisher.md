# Catalog Publisher Guide

Status: Draft (M3)

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