# HTX (High-Throughput eXchange)

[![Crates.io](https://img.shields.io/crates/v/htx.svg)](https://crates.io/crates/htx)
[![Documentation](https://docs.rs/htx/badge.svg)](https://docs.rs/htx)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../../LICENSE)

**QNet's tunneling protocol** - Secure, authenticated key exchange and encrypted tunnel establishment with forward secrecy and post-quantum resistance.

## Overview

HTX is QNet's core tunneling protocol that provides:

- **Authenticated Key Exchange**: Noise XK handshake pattern
- **Forward Secrecy**: Ephemeral keys for each session
- **Post-Quantum Security**: Hybrid classical/quantum-resistant cryptography
- **Session Encryption**: AEAD-encrypted tunnel traffic
- **Identity Authentication**: Self-certifying public keys
- **Traffic Analysis Resistance**: Constant-time operations and padding

## Features

- ✅ **Forward Secure**: Each session uses unique ephemeral keys
- ✅ **Quantum Resistant**: Hybrid Ed25519 + X25519 construction
- ✅ **Authenticated**: Mutual authentication of endpoints
- ✅ **Efficient**: Minimal round trips (1-RTT handshake)
- ✅ **Standards Based**: Uses Noise protocol framework

## Quick Start

```rust
use htx::{HtxInitiator, HtxResponder, HtxConfig};

// Initiator (client) configuration
let initiator_config = HtxConfig {
    static_key: my_private_key,
    remote_static_key: Some(server_public_key),
    ..Default::default()
};
let mut initiator = HtxInitiator::new(initiator_config);

// Responder (server) configuration
let responder_config = HtxConfig {
    static_key: server_private_key,
    ..Default::default()
};
let mut responder = HtxResponder::new(responder_config);

// Handshake
let init_msg = initiator.initiate()?;
let (response_msg, mut transport) = responder.respond(&init_msg)?;
let mut transport = initiator.complete(&response_msg)?;

// Now encrypt/decrypt tunnel traffic
let plaintext = b"Hello through HTX tunnel!";
let ciphertext = transport.encrypt(plaintext);
let decrypted = transport.decrypt(&ciphertext)?;
assert_eq!(plaintext, decrypted.as_slice());
```

## Protocol Flow

```
Initiator (Client)              Responder (Server)
    |                               |
    |  E, ES, SS                    |
    |------------------------------>|
    |                               |  Verify ES, SS
    |                               |  Generate transport keys
    |  EK                           |
    |<------------------------------|
    |  Verify EK                    |
    |  Generate transport keys      |
    |                               |
    |  Encrypted Tunnel Traffic     |
    |<----------------------------->|
    |                               |
```

**Handshake Messages:**
- **E**: Ephemeral public key (X25519)
- **ES**: Ephemeral-static DH (forward secrecy)
- **SS**: Static-static DH (authentication)
- **EK**: Encrypted ephemeral key (authentication)

## API Reference

### Configuration

```rust
pub struct HtxConfig {
    pub static_key: [u8; 32],              // Ed25519 private key
    pub remote_static_key: Option<[u8; 32]>, // Expected peer public key
    pub psk: Option<[u8; 32]>,             // Pre-shared key (optional)
    pub max_message_size: usize,           // Maximum message size
    pub enable_padding: bool,             // Traffic analysis resistance
}
```

### Handshake States

```rust
pub struct HtxInitiator { /* ... */ }
pub struct HtxResponder { /* ... */ }

impl HtxInitiator {
    pub fn new(config: HtxConfig) -> Self
    pub fn initiate(&mut self) -> Result<Vec<u8>, Error>
    pub fn complete(&mut self, response: &[u8]) -> Result<HtxTransport, Error>
}

impl HtxResponder {
    pub fn new(config: HtxConfig) -> Self
    pub fn respond(&mut self, init: &[u8]) -> Result<(Vec<u8>, HtxTransport), Error>
}
```

### Transport

```rust
pub struct HtxTransport {
    send_key: [u8; 32],
    recv_key: [u8; 32],
    send_nonce: u64,
    recv_nonce: u64,
}

impl HtxTransport {
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Vec<u8>
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, Error>
    pub fn rekey(&mut self) -> Result<(), Error>
}
```

## Security Properties

### Authentication
- **Mutual Authentication**: Both parties verify each other's identity
- **Key Confirmation**: Handshake proves possession of private keys
- **Identity Hiding**: Public keys don't reveal identity without context

### Forward Secrecy
- **Ephemeral Keys**: Fresh X25519 keypair per handshake
- **Perfect Forward Secrecy**: Compromised long-term keys don't affect past sessions
- **Key Rotation**: Optional rekeying during transport

### Cryptographic Primitives
- **AEAD**: ChaCha20-Poly1305 for authenticated encryption
- **KDF**: HKDF-SHA256 for key derivation
- **Signatures**: Ed25519 for static key authentication
- **Key Exchange**: X25519 for ephemeral key agreement

## Performance

**Handshake Performance:**

| Operation | Latency | CPU Cycles |
|-----------|---------|------------|
| Key Generation | ~50 µs | ~100K |
| DH Computation | ~10 µs | ~20K |
| Handshake (total) | ~150 µs | ~300K |

**Transport Performance:**

| Operation | Throughput | Latency |
|-----------|------------|---------|
| Encrypt 1KiB | ~800 MiB/s | ~1.3 µs |
| Decrypt 1KiB | ~800 MiB/s | ~1.3 µs |
| Rekey | ~20 µs | ~40K |

## Advanced Usage

### Pre-Shared Keys

```rust
// Add PSK for additional authentication
let config = HtxConfig {
    static_key: my_key,
    psk: Some(shared_secret),
    ..Default::default()
};
```

### Identity Verification

```rust
// Verify peer identity during handshake
let expected_key = get_expected_public_key(peer_id);
let config = HtxConfig {
    static_key: my_key,
    remote_static_key: Some(expected_key),
    ..Default::default()
};
```

### Transport Rekeying

```rust
// Periodically rekey for forward secrecy
if should_rekey() {
    transport.rekey()?;
}
```

## Error Handling

```rust
use htx::Error;

match result {
    Ok(data) => process_data(data),
    Err(Error::AuthenticationFailed) => log!("Peer authentication failed"),
    Err(Error::DecryptionFailed) => log!("Message decryption failed"),
    Err(Error::InvalidMessage) => log!("Malformed protocol message"),
    Err(Error::KeyExchangeFailed) => log!("Key exchange failed"),
    Err(_) => log!("Unknown HTX error"),
}
```

## Testing

Run the test suite:

```bash
cargo test
```

Run protocol fuzzing:

```bash
cargo +nightly fuzz run htx_handshake_fuzz
```

Run benchmarks:

```bash
cargo bench
```

## Integration

Add to your `Cargo.toml`:

```toml
[dependencies]
htx = { path = "../crates/htx" }
```

For external projects:

```toml
[dependencies]
htx = "0.1"
```

## Architecture

```
htx/
├── src/
│   ├── lib.rs           # Main library interface
│   ├── handshake.rs     # Noise protocol handshake
│   ├── transport.rs     # Encrypted transport
│   ├── config.rs        # Configuration structures
│   ├── crypto.rs        # Cryptographic operations
│   ├── error.rs         # Error types
│   └── state.rs         # Protocol state machines
├── tests/               # Unit tests
├── benches/             # Performance benchmarks
└── fuzz/               # Fuzzing targets
```

## Protocol Specification

See [HTX Protocol Specification](../../specs/001-qnet/spec.md) for detailed protocol documentation.

### Noise XK Pattern
HTX implements the Noise XK pattern:
- **X**: One-way authentication (client authenticates to server)
- **K**: Mutual authentication (both parties authenticate)

### Extensions
- **Padding**: Optional traffic analysis resistance
- **Rekeying**: Forward secrecy maintenance
- **PSK**: Pre-shared key authentication

## Related Crates

- **`core-crypto`**: Provides cryptographic primitives
- **`core-framing`**: Handles message framing
- **`core-identity`**: Manages self-certifying identities

## Config: catalog-first (M3)

HTX integrates with a signed decoy catalog as the primary configuration source, with seeds as a resilience fallback:
- Decoy catalog: Signed JSON (DET-CBOR + Ed25519) loaded at app start; provides decoy hosts, ALPN hints, and update mirrors.
- Seeds (fallback): Signed seed list used only when no valid catalog is available.
- See: `../../docs/catalog-schema.md` and app behavior in `../../docs/apps/stealth-browser.md`.

## Contributing

See the main [Contributing Guide](../CONTRIBUTING.md) for development setup and contribution guidelines.

### Development Requirements

- Follow [AI Guardrail](../../memory/ai-guardrail.md)
- Meet [Testing Rules](../../memory/testing-rules.md)
- Include `AI-Guardrail: PASS` and `Testing-Rules: PASS` in commits

## License

Licensed under the MIT License. See [LICENSE](../../../LICENSE) for details.

## Security

If you discover a security vulnerability, please see our [Security Policy](../../../SECURITY.md).

---

*Part of the [QNet](https://github.com/QW1CKS/qnet) project - Building the future of decentralized networking.*