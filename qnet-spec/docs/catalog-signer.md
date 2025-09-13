# Catalog Signer (CLI)

Status: M3 v1 (docs complete); CLI implemented under `crates/catalog-signer`

A small Rust CLI that turns YAML templates into a signed catalog ready to bundle with the app or publish to mirrors.

## Summary

Inputs:
- YAML decoys: `templates/decoys.yml`
- YAML metadata: `templates/catalog.meta.yml`
- Ed25519 private key (hex) via env: `CATALOG_PRIVKEY`

Outputs:
- `catalog.json` (outer JSON with signature_hex) or detached `catalog.json` + `catalog.json.sig`

Signature model:
- Compute DET-CBOR over the inner catalog object (without signature fields)
- Sign bytes with Ed25519 using the provided private key
- Embed as `signature_hex` or output a separate `.sig`

## CLI

```
# Build
cargo install --path crates/catalog-signer

# Sign
catalog-signer sign \
  --decoys qnet-spec/templates/decoys.yml \
  --meta qnet-spec/templates/catalog.meta.yml \
  --out dist/catalog.json \
  --sig dist/catalog.json.sig

# Verify (pinned public key)
catalog-signer verify \
  --catalog dist/catalog.json \
  --sig dist/catalog.json.sig \
  --pubkey-file qnet-spec/templates/keys/publisher.pub
```

Flags:
- `--inline` to write `signature_hex` inline instead of detached file
- `--expires` to override TTL window (e.g., `7d`)
- `--now` to set generation time (RFC3339) for reproducibility tests

## Deterministic CBOR (DET-CBOR)
- Canonical map key ordering (lexicographic by UTF-8 bytes)
- No indefinite-length encodings
- Stable floating/integer encodings

The signer must serialize the inner catalog object to DET-CBOR bytes and sign those exact bytes.

## Example

- Inputs: `qnet-spec/templates/decoys.yml`, `qnet-spec/templates/catalog.meta.yml`
- Outputs: `dist/catalog.json`, `dist/catalog.json.sig`
- Verify with: `qnet-spec/templates/keys/publisher.pub`

Note: For now, run the signer manually with `CATALOG_PRIVKEY` set to your private seed (store securely). When QNet scales, automate in CI with GitHub Secrets.

See also:
- `docs/catalog-schema.md` for the catalog format
- `docs/catalog-publisher.md` for CI publishing guidance
