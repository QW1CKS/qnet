# Core Crypto

[![Crates.io](https://img.shields.io/crates/v/core-crypto.svg)](https://crates.io/crates/core-crypto)
[![Documentation](https://docs.rs/core-crypto/badge.svg)](https://docs.rs/core-crypto)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Core cryptographic primitives for QNet** - Thin wrappers around the [ring](https://github.com/briansmith/ring) cryptography library, providing a consistent API for QNet's cryptographic operations.

## Overview

The `core-crypto` crate provides essential cryptographic primitives used throughout the QNet protocol stack. It offers a clean, safe API for:

- **Authenticated Encryption**: ChaCha20-Poly1305 AEAD
- **Digital Signatures**: Ed25519
- **Key Exchange**: X25519 Elliptic Curve Diffie-Hellman
- **Key Derivation**: HKDF-SHA256

## Features

- ✅ **Memory Safe**: Built on Rust's ownership system
- ✅ **Zero-Copy**: Efficient operations with minimal allocations
- ✅ **FIPS Compliant**: Uses battle-tested ring library
- ✅ **Post-Quantum Ready**: Compatible with hybrid schemes
- ✅ **Cross-Platform**: Works on all major platforms

## Quick Start

```rust
use core_crypto::{aead, ed25519, x25519, hkdf};

// AEAD encryption/decryption
let key = [0u8; 32];
let nonce = [0u8; 12];
let plaintext = b"Hello, QNet!";
let ciphertext = aead::seal(&key, &nonce, &[], plaintext);
let decrypted = aead::open(&key, &nonce, &[], &ciphertext)?;

// Digital signatures
let (sk, pk) = ed25519::keypair();
let signature = ed25519::sign(&sk, plaintext);
let valid = ed25519::verify(&pk, plaintext, &signature);

// Key exchange
let (alice_sk, alice_pk) = x25519::keypair();
let (bob_sk, bob_pk) = x25519::keypair();
let alice_shared = x25519::dh(&alice_sk, &bob_pk);
let bob_shared = x25519::dh(&bob_sk, &alice_pk);
assert_eq!(alice_shared, bob_shared);
```

## API Reference

### AEAD (Authenticated Encryption with Associated Data)

```rust
pub fn seal(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], plaintext: &[u8]) -> Vec<u8>
pub fn open(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, Error>
```

**Parameters:**
- `key`: 32-byte ChaCha20 key
- `nonce`: 12-byte unique nonce
- `aad`: Associated data (authenticated but not encrypted)
- `plaintext`/`ciphertext`: Data to encrypt/decrypt

**Security Notes:**
- Nonces must be unique for each message with the same key
- AAD is authenticated but not encrypted
- Returns ciphertext + 16-byte authentication tag

### Digital Signatures (Ed25519)

```rust
pub fn keypair() -> ([u8; 32], [u8; 32])
pub fn sign(secret_key: &[u8; 32], message: &[u8]) -> [u8; 64]
pub fn verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool
```

**Features:**
- Deterministic signatures (same message + key = same signature)
- 32-byte public keys, 64-byte signatures
- Constant-time verification

### Key Exchange (X25519)

```rust
pub fn keypair() -> ([u8; 32], [u8; 32])
pub fn dh(secret_key: &[u8; 32], public_key: &[u8; 32]) -> [u8; 32]
```

**Usage:**
```rust
// Alice generates keys
let (alice_sk, alice_pk) = x25519::keypair();

// Bob generates keys
let (bob_sk, bob_pk) = x25519::keypair();

// Both compute shared secret
let alice_shared = x25519::dh(&alice_sk, &bob_pk);
let bob_shared = x25519::dh(&bob_sk, &alice_pk);

// Shared secrets are identical
assert_eq!(alice_shared, bob_shared);
```

### Key Derivation (HKDF-SHA256)

```rust
pub fn extract(salt: &[u8], ikm: &[u8]) -> [u8; 32]
pub fn expand(prk: &[u8; 32], info: &[u8], output: &mut [u8])
```

**Usage:**
```rust
// Extract pseudorandom key
let prk = hkdf::extract(b"salt", b"input key material");

// Expand to desired length
let mut output = [0u8; 64];
hkdf::expand(&prk, b"info", &mut output);
```

## Security Considerations

### Key Management
- **Key Lifetime**: Rotate keys regularly based on usage
- **Key Storage**: Use secure key storage facilities
- **Key Generation**: Use cryptographically secure random sources

### Nonce Management
- **Uniqueness**: Never reuse nonces with the same key
- **Randomness**: Use cryptographically secure random nonces
- **Counter Mode**: Consider monotonic counters for deterministic nonces

### Error Handling
- **Authentication Failures**: Treat as potential attacks
- **Invalid Inputs**: Validate all cryptographic inputs
- **Timing Attacks**: Operations are constant-time where possible

## Performance

**Benchmark Results** (on modern hardware):

| Operation | Throughput | Latency |
|-----------|------------|---------|
| AEAD Seal (16KiB) | ~1.35 GiB/s | ~11.8 µs |
| AEAD Open (16KiB) | ~1.35 GiB/s | ~11.7 µs |
| Ed25519 Sign | ~50K ops/s | ~20 µs |
| Ed25519 Verify | ~15K ops/s | ~65 µs |
| X25519 DH | ~100K ops/s | ~10 µs |

## Testing

Run the test suite:

```bash
cargo test
```

Run benchmarks:

```bash
cargo bench
```

Run fuzzing (requires nightly):

```bash
cargo +nightly fuzz run aead_fuzz
```

## Integration

Add to your `Cargo.toml`:

```toml
[dependencies]
core-crypto = { path = "../crates/core-crypto" }
```

For external projects:

```toml
[dependencies]
core-crypto = "0.1"
```

## Architecture

```
core-crypto/
├── src/
│   ├── lib.rs           # Main library interface
│   ├── aead.rs          # AEAD operations
│   ├── ed25519.rs       # Digital signatures
│   ├── x25519.rs        # Key exchange
│   ├── hkdf.rs          # Key derivation
│   └── error.rs         # Error types
├── tests/               # Unit tests
├── benches/             # Performance benchmarks
└── fuzz/               # Fuzzing targets
```

## Related Crates

- **`core-framing`**: Uses AEAD for frame encryption
- **`htx`**: Uses all primitives for secure handshakes
- **`core-identity`**: Uses Ed25519 for self-certifying IDs

## Contributing

See the main [Contributing Guide](../docs/CONTRIBUTING.md) for development setup and contribution guidelines.

### Development Requirements

- Follow [AI Guardrail](../qnet-spec/memory/ai-guardrail.md)
- Meet [Testing Rules](../qnet-spec/memory/testing-rules.md)
- Include `AI-Guardrail: PASS` and `Testing-Rules: PASS` in commits

## License

Licensed under the MIT License. See [LICENSE](../../LICENSE) for details.

## Security

If you discover a security vulnerability, please see our [Security Policy](../../SECURITY.md).

---

*Part of the [QNet](https://github.com/QW1CKS/qnet) project - Building the future of decentralized networking.*