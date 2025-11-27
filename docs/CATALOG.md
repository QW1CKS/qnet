# QNet Catalog Management Guide

Comprehensive guide for signing, distributing, and managing QNet decoy catalogs.

## 📋 Table of Contents

- [Overview](#overview)
- [Catalog Format](#catalog-format)
- [Signing Catalogs](#signing-catalogs)
- [Distribution](#distribution)
- [Verification](#verification)
- [Rotation Policy](#rotation-policy)

---

## 🌐 Overview

### What is a Decoy Catalog?

The **decoy catalog** is a signed JSON file containing:
- List of trusted HTTPS domains for HTX traffic masking
- TLS fingerprint templates (JA3, ALPN, cipher suites)
- Catalog version and expiration timestamp
- Ed25519 signature for integrity verification

**Purpose:**
- Provide pre-vetted masking targets
- Ensure traffic indistinguishability
- Enable automatic updates
- Prevent catalog tampering

### Catalog vs. Seeds

**Catalog-first model (current):**
```
User starts → Load signed catalog → Connect via catalog decoys
              ↓ (if unavailable)
              Load hardcoded seeds → Connect via seed decoys
```

**Priority:**
1. Fresh signed catalog (preferred)
2. Cached catalog (if not expired)
3. Hardcoded seeds (fallback only)

---

## 📄 Catalog Format

### JSON Schema

See full specification: [qnet-spec/docs/catalog-schema.md](../qnet-spec/docs/catalog-schema.md)

**Example catalog:**
```json
{
  "catalog_version": 3,
  "published_at": "2025-11-27T12:00:00Z",
  "expires_at": "2025-12-27T12:00:00Z",
  "decoys": [
    {
      "domain": "example.com",
      "ip": "93.184.216.34",
      "port": 443,
      "sni": "example.com",
      "ja3_fingerprint": "771,4865-4866-4867...",
      "alpn": ["h2", "http/1.1"],
      "cipher_suites": [
        "TLS_AES_128_GCM_SHA256",
        "TLS_AES_256_GCM_SHA384"
      ],
      "tls_version": "1.3",
      "verify_cert": true
    }
  ],
  "signature_hex": "a1b2c3d4..."
}
```

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `catalog_version` | integer | Monotonically increasing version |
| `published_at` | ISO 8601 | Catalog creation timestamp |
| `expires_at` | ISO 8601 | Expiration timestamp (30 days recommended) |
| `decoys` | array | List of decoy configurations |
| `signature_hex` | hex string | Ed25519 signature of canonical JSON |

### Decoy Entry Fields

| Field | Required | Description |
|-------|----------|-------------|
| `domain` | ✅ | Target domain (e.g., "example.com") |
| `ip` | ✅ | IPv4 address |
| `port` | ✅ | Port number (usually 443) |
| `sni` | ✅ | SNI hostname |
| `ja3_fingerprint` | ❌ | TLS fingerprint (optional) |
| `alpn` | ❌ | ALPN protocols |
| `cipher_suites` | ❌ | TLS cipher suites |
| `tls_version` | ❌ | TLS version (default: "1.3") |
| `verify_cert` | ❌ | Verify certificate (default: true) |

---

## 🔐 Signing Catalogs

### Prerequisites

**Install catalog-signer:**
```bash
cargo build -p catalog-signer --release
cp target/release/catalog-signer /usr/local/bin/
```

**Generate signing key (once):**
```bash
# Generate Ed25519 keypair
catalog-signer keygen --output qnet-catalog-key

# Output:
# - qnet-catalog-key.private (KEEP SECRET!)
# - qnet-catalog-key.public  (distribute to users)
```

**⚠️ KEY SECURITY:**
- Store private key in encrypted storage
- Use hardware security module (HSM) for production
- Never commit private key to git
- Rotate keys annually

### Signing Process

**1. Create unsigned catalog:**
```json
{
  "catalog_version": 4,
  "published_at": "2025-11-27T12:00:00Z",
  "expires_at": "2025-12-27T12:00:00Z",
  "decoys": [
    {
      "domain": "example.com",
      "ip": "93.184.216.34",
      "port": 443,
      "sni": "example.com"
    }
  ]
}
```

**2. Sign catalog:**
```bash
# Set private key path
export CATALOG_PRIVKEY=/path/to/qnet-catalog-key.private

# Sign catalog
catalog-signer sign \
  --input unsigned-catalog.json \
  --output signed-catalog.json

# Verify signature
catalog-signer verify \
  --input signed-catalog.json \
  --pubkey qnet-catalog-key.public
```

**3. Validate output:**
```bash
# Check signature field added
jq '.signature_hex' signed-catalog.json

# Verify version incremented
jq '.catalog_version' signed-catalog.json
```

### Automated Signing

**CI/CD Pipeline (GitHub Actions):**
```yaml
name: Sign Catalog

on:
  push:
    paths:
      - 'catalogs/catalog.json'

jobs:
  sign:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Build catalog-signer
        run: cargo build -p catalog-signer --release
      
      - name: Sign catalog
        env:
          CATALOG_PRIVKEY: ${{ secrets.CATALOG_PRIVATE_KEY }}
        run: |
          target/release/catalog-signer sign \
            --input catalogs/catalog.json \
            --output catalogs/catalog-signed.json
      
      - name: Upload artifact
        uses: actions/upload-artifact@v3
        with:
          name: signed-catalog
          path: catalogs/catalog-signed.json
```

---

## 📤 Distribution

### HTTP Distribution

**Recommended hosting:**
```
https://catalog.qnet.network/catalog.json
https://catalog-backup.qnet.network/catalog.json
```

**Nginx configuration:**
```nginx
server {
    listen 443 ssl http2;
    server_name catalog.qnet.network;

    ssl_certificate /etc/letsencrypt/live/catalog.qnet.network/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/catalog.qnet.network/privkey.pem;

    location /catalog.json {
        alias /var/www/qnet/catalog-signed.json;
        add_header Content-Type application/json;
        add_header Cache-Control "public, max-age=3600";
        add_header Access-Control-Allow-Origin "*";
    }

    location /catalog.json.sig {
        alias /var/www/qnet/catalog-signed.json.sig;
        add_header Content-Type text/plain;
    }
}
```

### IPFS Distribution

**Pin to IPFS:**
```bash
# Add catalog to IPFS
ipfs add catalog-signed.json

# Output: QmHash...

# Pin to ensure persistence
ipfs pin add QmHash...

# Publish to IPNS (human-readable name)
ipfs name publish QmHash...
```

**Helper configuration:**
```bash
# Environment variable
export QNET_CATALOG_URL="https://ipfs.io/ipfs/QmHash.../catalog.json"
```

### GitHub Releases

**Attach to releases:**
```bash
gh release create v1.2.3 \
  --title "QNet v1.2.3" \
  --notes "Updated catalog" \
  catalog-signed.json
```

**Users download:**
```bash
curl -L https://github.com/QW1CKS/qnet/releases/latest/download/catalog-signed.json \
  -o ~/.qnet/catalog.json
```

---

## ✅ Verification

### Manual Verification

```bash
# Verify with catalog-signer
catalog-signer verify \
  --input catalog-signed.json \
  --pubkey qnet-catalog-key.public

# Expected output:
# ✅ Signature valid
# ✅ Catalog version: 4
# ✅ Expires: 2025-12-27T12:00:00Z
```

### Programmatic Verification

**Rust example:**
```rust
use core_crypto::signatures::verify_ed25519;
use core_cbor::deterministic_encode;

fn verify_catalog(catalog_json: &str, pubkey: &[u8; 32]) -> bool {
    let mut catalog: serde_json::Value = serde_json::from_str(catalog_json)?;
    let sig_hex = catalog["signature_hex"].as_str()?;
    
    // Remove signature for verification
    catalog.as_object_mut()?.remove("signature_hex");
    
    // Canonicalize to DET-CBOR
    let canonical = deterministic_encode(&catalog)?;
    
    // Verify Ed25519 signature
    let sig = hex::decode(sig_hex)?;
    verify_ed25519(&canonical, &sig, pubkey)
}
```

### Verification in Helper

**Automatic verification on load:**
```rust
// Helper startup
let catalog = load_catalog()?;

if !verify_catalog_signature(&catalog) {
    warn!("Catalog signature invalid! Falling back to seeds.");
    return load_hardcoded_seeds();
}

if catalog.is_expired() {
    warn!("Catalog expired. Checking for updates...");
    // Attempt update
}
```

---

## 🔄 Rotation Policy

### Update Frequency

**Recommended schedule:**
- **Weekly**: Add/remove decoys
- **Monthly**: Rotate catalog version
- **Quarterly**: Rotate signing keys

### Catalog Versioning

**Version numbering:**
```
v1: Initial catalog
v2: Add 3 new decoys
v3: Remove expired decoy
v4: Update TLS fingerprints
```

**Monotonic increment:**
- Always increment version number
- Never reuse version numbers
- Reject catalogs with lower versions

### Expiration Handling

**Expiration timeline:**
```
Published: 2025-11-27
Expires:   2025-12-27 (30 days)
Grace:     2025-12-29 (2 days)
Hard fail: 2026-01-03 (7 days total)
```

**Helper behavior:**
- 7+ days before expiry: Use catalog normally
- 7-0 days: Warn user, attempt update
- 0-2 days: Grace period, continue using
- 2+ days: Fallback to seeds

### Signing Key Rotation

**Annual key rotation:**
```bash
# Generate new keypair
catalog-signer keygen --output qnet-catalog-key-2026

# Sign catalog with new key
export CATALOG_PRIVKEY=qnet-catalog-key-2026.private
catalog-signer sign --input catalog.json --output catalog-v5.json

# Distribute new public key to users
# - Update default pubkey in code
# - Announce on website/GitHub
```

**Overlap period:**
- Support old key for 30 days
- Helpers accept both keys during transition
- Remove old key after 60 days

---

## 📊 Best Practices

### Decoy Selection Criteria

**Choose decoys that are:**
- ✅ High-traffic sites (blend in)
- ✅ TLS 1.3 with modern ciphers
- ✅ HTTP/2 or HTTP/3 support
- ✅ Globally accessible
- ✅ Stable IP addresses

**Avoid:**
- ❌ Controversial sites (draws attention)
- ❌ Sites with captchas
- ❌ CDN-only sites (no fixed IP)
- ❌ Sites blocking Tor/VPNs

### Catalog Diversity

**Aim for:**
- 20-50 decoys per catalog
- Mix of CDN providers
- Geographic diversity
- TLS fingerprint variety

### Testing New Decoys

```bash
# Test decoy connectivity
curl -I https://example.com --http2

# Check TLS fingerprint
openssl s_client -connect example.com:443 -servername example.com

# Verify reachability from multiple locations
# Use: https://www.uptrends.com/tools/uptime
```

---

## 🆘 Troubleshooting

### Signature Verification Fails

**Causes:**
- Wrong public key
- Catalog modified after signing
- Encoding issues (CRLF vs LF)

**Fix:**
```bash
# Re-sign catalog
catalog-signer sign --input catalog.json --output catalog-signed.json

# Verify with correct pubkey
catalog-signer verify --input catalog-signed.json --pubkey correct-key.public
```

### Catalog Won't Load

**Check format:**
```bash
# Validate JSON
jq . catalog-signed.json

# Check required fields
jq '{version:.catalog_version, expires:.expires_at}' catalog-signed.json
```

### Version Rollback Detected

**Symptoms:**
- Helper refuses to load catalog
- Error: "Catalog version regression"

**Fix:**
- Increment `catalog_version` to higher than previous
- Never decrement version numbers

---

## 📞 Support

- **Catalog issues**: https://github.com/QW1CKS/qnet/issues
- **Spec reference**: [qnet-spec/docs/catalog-schema.md](../qnet-spec/docs/catalog-schema.md)
- **Signer tool**: `cargo doc -p catalog-signer --open`

---

**Secure catalog management = Reliable network operation** 🔒
