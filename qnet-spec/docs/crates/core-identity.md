# Core Identity

[![Crates.io](https://img.shields.io/crates/v/core-identity.svg)](https://crates.io/crates/core-identity)
[![Documentation](https://docs.rs/core-identity/badge.svg)](https://docs.rs/core-identity)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Self-certifying identity system for QNet** - Decentralized identity management with cryptographic self-certification, reputation systems, and privacy-preserving authentication.

## Overview

The `core-identity` crate provides QNet's identity and authentication layer:

- **Self-Certifying IDs**: Public keys as identities
- **Reputation System**: Trust and reputation management
- **Privacy Preservation**: Anonymous authentication
- **Certificate Management**: Identity certificates and chains
- **Access Control**: Decentralized authorization
- **Key Management**: Secure key storage and rotation

## Features

- ✅ **Self-Sovereign**: Users control their own identities
- ✅ **Privacy-First**: Anonymous and pseudonymous operation
- ✅ **Decentralized**: No central identity authority
- ✅ **Scalable**: Efficient identity operations
- ✅ **Secure**: Cryptographically secure identity proofs

## Quick Start

```rust
use core_identity::{Identity, IdentityManager, ReputationManager};

// Create a new identity
let identity = Identity::new()?;
println!("Identity ID: {}", identity.id());

// Create identity manager
let mut manager = IdentityManager::new();

// Register identity
manager.register(identity.clone())?;

// Create reputation manager
let mut reputation = ReputationManager::new();

// Update reputation
reputation.update_reputation(identity.id(), 0.8)?;

// Verify identity
let is_valid = manager.verify_identity(identity.id())?;
assert!(is_valid);
```

## API Reference

### Identity Structure

```rust
pub struct Identity {
    id: IdentityId,
    public_key: [u8; 32],        // Ed25519 public key
    created_at: Timestamp,
    metadata: IdentityMetadata,
}

impl Identity {
    pub fn new() -> Result<Self, Error>
    pub fn from_keypair(sk: &[u8; 32], pk: &[u8; 32]) -> Self
    pub fn id(&self) -> &IdentityId
    pub fn public_key(&self) -> &[u8; 32]
    pub fn sign(&self, message: &[u8]) -> Result<Signature, Error>
    pub fn verify(&self, message: &[u8], signature: &[u8; 64]) -> bool
}
```

### Identity Manager

```rust
pub struct IdentityManager {
    identities: HashMap<IdentityId, Identity>,
    certificates: CertificateStore,
}

impl IdentityManager {
    pub fn new() -> Self
    pub fn register(&mut self, identity: Identity) -> Result<(), Error>
    pub fn lookup(&self, id: &IdentityId) -> Option<&Identity>
    pub fn verify_identity(&self, id: &IdentityId) -> Result<bool, Error>
    pub fn revoke_identity(&mut self, id: &IdentityId) -> Result<(), Error>
}
```

### Reputation System

```rust
pub struct ReputationManager {
    scores: HashMap<IdentityId, f64>,
    history: ReputationHistory,
}

impl ReputationManager {
    pub fn new() -> Self
    pub fn get_reputation(&self, id: &IdentityId) -> Option<f64>
    pub fn update_reputation(&mut self, id: &IdentityId, score: f64) -> Result<(), Error>
    pub fn calculate_trust_score(&self, id: &IdentityId) -> f64
}
```

## Identity Types

### Self-Certifying Identities

**Properties:**
- **Public Key = Identity**: No separate identity namespace
- **Cryptographic Proof**: Possession proves identity
- **Decentralized**: No central registration authority
- **Collision Resistant**: SHA256 hash of public key

**Format:**
```
Identity ID: SHA256(public_key)[0..20]  // 20-byte truncated hash
Public Key: 32 bytes (Ed25519)
Signature: 64 bytes (Ed25519)
```

### Identity Certificates

```rust
pub struct Certificate {
    subject: IdentityId,
    issuer: IdentityId,
    claims: Vec<Claim>,
    signature: Signature,
    expires_at: Timestamp,
}

pub enum Claim {
    Name(String),
    Email(String),
    Organization(String),
    Role(String),
    Custom(String, String),
}
```

### Reputation Scores

**Scoring Algorithm:**
- **Direct Reputation**: Explicit ratings from peers
- **Indirect Reputation**: Transitive trust through networks
- **Behavioral Reputation**: Observed behavior patterns
- **Temporal Decay**: Reputation ages over time

**Score Range:** 0.0 (untrusted) to 1.0 (fully trusted)

## Security Considerations

### Identity Security
- **Key Protection**: Secure private key storage
- **Key Rotation**: Periodic key rotation
- **Revocation**: Certificate revocation lists
- **Recovery**: Identity recovery mechanisms

### Privacy Protection
- **Anonymous Operation**: No personal information required
- **Pseudonymous IDs**: Optional persistent pseudonyms
- **Metadata Minimization**: Minimal identity metadata
- **Traffic Analysis**: Resistance to traffic analysis

### Authentication
- **Mutual Authentication**: Both parties verify identities
- **Freshness**: Timestamps prevent replay attacks
- **Context Binding**: Signatures bound to specific contexts

## Advanced Usage

### Certificate Chains

```rust
// Create certificate chain
let root_cert = create_root_certificate(authority_key)?;
let intermediate_cert = create_intermediate_certificate(intermediate_key, &root_cert)?;
let user_cert = create_user_certificate(user_key, &intermediate_cert)?;

// Verify certificate chain
let is_valid = verify_certificate_chain(&user_cert, &[intermediate_cert, root_cert])?;
```

### Reputation-Based Access Control

```rust
// Check reputation before granting access
let reputation = reputation_manager.get_reputation(&identity_id)?;
if reputation > 0.7 {
    grant_access(identity_id)?;
} else {
    deny_access("Insufficient reputation")?;
}
```

### Anonymous Authentication

```rust
// Create anonymous credential
let credential = AnonymousCredential::new(&identity)?;

// Prove possession without revealing identity
let proof = credential.prove_possession(challenge)?;
let is_valid = verify_anonymous_proof(&proof, challenge)?;
```

## Performance

**Identity Operations:**

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Identity Creation | ~100 µs | ~10K ops/s |
| Signature Verification | ~50 µs | ~20K ops/s |
| Reputation Lookup | ~10 µs | ~100K ops/s |
| Certificate Verification | ~200 µs | ~5K ops/s |

**Storage Requirements:**
- Identity: ~100 bytes
- Certificate: ~500 bytes
- Reputation Record: ~50 bytes

## Error Handling

```rust
use core_identity::Error;

match result {
    Ok(data) => process_data(data),
    Err(Error::InvalidIdentity) => log!("Invalid identity format"),
    Err(Error::VerificationFailed) => log!("Cryptographic verification failed"),
    Err(Error::IdentityNotFound) => log!("Identity not found"),
    Err(Error::ReputationTooLow) => log!("Insufficient reputation"),
    Err(Error::CertificateExpired) => log!("Certificate has expired"),
    Err(_) => log!("Unknown identity error"),
}
```

## Testing

Run the test suite:

```bash
cargo test
```

Run cryptographic tests:

```bash
cargo test --features crypto
```

Run benchmarks:

```bash
cargo bench
```

## Integration

Add to your `Cargo.toml`:

```toml
[dependencies]
core-identity = { path = "../crates/core-identity" }
```

For external projects:

```toml
[dependencies]
core-identity = "0.1"
```

## Architecture

```
core-identity/
├── src/
│   ├── lib.rs           # Main library interface
│   ├── identity.rs      # Identity structures
│   ├── manager.rs       # Identity management
│   ├── reputation.rs    # Reputation system
│   ├── certificate.rs   # Certificate handling
│   ├── crypto.rs        # Cryptographic operations
│   ├── error.rs         # Error types
│   └── storage.rs       # Identity storage
├── tests/               # Unit tests
├── benches/             # Performance benchmarks
└── examples/            # Usage examples
```

## Related Crates

- **`core-crypto`**: Provides Ed25519 signatures
- **`core-governance`**: Uses identities for voting
- **`core-routing`**: Identity-based routing decisions

## Contributing

See the main [Contributing Guide](../CONTRIBUTING.md) for development setup and contribution guidelines.

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