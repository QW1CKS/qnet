# QNet Production Hardening - Implementation Plan

## Overview

This plan addresses all audit findings to transform QNet from a functional prototype (~30% complete) to a production-ready overlay network foundation.

## Timeline Estimate

**Total Duration**: 6-8 weeks (with 1 developer, full-time)

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| Phase 1: TLS Fingerprinting | 2 weeks | None |
| Phase 2: Refactoring | 1 week | None (can parallel with Phase 1) |
| Phase 3: Testing Infrastructure | 2 weeks | Phase 1, 2 complete |
| Phase 4: Documentation | 1 week | Phase 1, 2 complete |
| Phase 5: Polish & Benchmarks | 1-2 weeks | All phases |

---

## Phase 1: Fix TLS Fingerprint Mirroring (🔴 Critical)

### Problem

Current implementation uses simplified JA3 approximation:
- Missing TLS version and cipher suites
- Uses HTTP probes instead of TLS handshake inspection
- Hardcoded defaults don't match real targets

### Solution Architecture

#### 1.1 Complete JA3/JA4 Implementation

**New Module**: `crates/htx/src/fingerprint.rs`

```rust
pub struct TlsFingerprint {
    pub ja3: String,           // MD5 hash of JA3 string
    pub ja4: String,           // JA4 format
    pub version: TlsVersion,   // TLS 1.2, 1.3
    pub cipher_suites: Vec<u16>,
    pub extensions: Vec<u16>,
    pub groups: Vec<u16>,
    pub ec_point_formats: Vec<u8>,
    pub alpn: Vec<String>,
}

// Full JA3 string format:
// SSLVersion,Ciphers,Extensions,EllipticCurves,EllipticCurvePointFormats
pub fn compute_ja3_full(fp: &TlsFingerprint) -> String;

// JA4 format (newer, more robust)
pub fn compute_ja4(fp: &TlsFingerprint) -> String;
```

#### 1.2 TLS Handshake Inspection

**Approach**: Use `rustls` with custom `ServerCertVerifier` to intercept handshake

```rust
struct FingerprintExtractor {
    captured: Arc<Mutex<Option<TlsFingerprint>>>,
}

impl ServerCertVerifier for FingerprintExtractor {
    fn verify_server_cert(&self, ...) -> Result<...> {
        // Extract server's chosen cipher, version, extensions
        // Store in captured mutex
        // Return Ok to continue handshake
    }
}
```

**Integration**:
```rust
pub fn calibrate_tls_fingerprint(origin: &str) -> Result<TlsFingerprint> {
    // 1. Create rustls client with FingerprintExtractor
    // 2. Perform TLS handshake
    // 3. Extract captured fingerprint
    // 4. Cache with TTL
}
```

#### 1.3 Update `tls_mirror.rs`

**Changes**:
- Replace `compute_ja3()` with `compute_ja3_full()`
- Replace `calibrate()` HTTP probe with `calibrate_tls_fingerprint()`
- Update `Template` struct to include all JA3 components
- Add `JA4` support

**Files Modified**:
- `crates/htx/src/tls_mirror.rs`
- `crates/htx/src/fingerprint.rs` (new)
- `crates/htx/Cargo.toml` (add dependencies)

---

## Phase 2: Refactor stealth-browser (🔴 Critical)

### Current State

Single 1559-line `main.rs` with mixed concerns.

### Target Architecture

```
apps/stealth-browser/src/
├── main.rs           (80 lines - entry point only)
├── lib.rs            (100 lines - AppState, core types)
├── config.rs         (150 lines - Config, CLI parsing)
├── catalog.rs        (200 lines - CatalogState, updates)
├── socks.rs          (400 lines - SOCKS5 proxy handler)
├── status.rs         (400 lines - Status server, endpoints)
└── constants.rs      (50 lines - Magic numbers)
```

### Migration Plan

#### 2.1 Extract Constants

**New File**: `constants.rs`

```rust
// Timing
pub const CATALOG_UPDATE_INTERVAL_SECS: u64 = 600; // 10 minutes
pub const STATUS_POLL_INTERVAL_MS: u64 = 1600;
pub const CONNECTIVITY_CHECK_INTERVAL_SECS: u64 = 5;
pub const STALE_CONNECTION_THRESHOLD_MS: u64 = 9000;

// Ports (defaults)
pub const DEFAULT_SOCKS_PORT: u16 = 1088;
pub const DEFAULT_STATUS_PORT: u16 = 8088;

// Timeouts
pub const STATUS_READ_TIMEOUT_MS: u64 = 900;
pub const STATUS_WRITE_TIMEOUT_SECS: u64 = 2;
pub const SEED_CONNECT_TIMEOUT_SECS: u64 = 3;
```

#### 2.2 Split Modules

**Steps**:
1. Create `lib.rs` with `AppState`, `StatusSnapshot`, shared types
2. Move `Config` and CLI parsing to `config.rs`
3. Move catalog logic to `catalog.rs`
4. Move SOCKS5 handler to `socks.rs`
5. Move status server to `status.rs`
6. Update `main.rs` to orchestrate modules

**Testing**: After each step, run `cargo test` to ensure no breakage

---

## Phase 3: Testing Infrastructure (⚠️ Medium)

### 3.1 Integration Tests

**New Directory**: `apps/stealth-browser/tests/`

```
tests/
├── integration/
│   ├── catalog_lifecycle.rs
│   ├── socks_masked_mode.rs
│   ├── status_api.rs
│   └── single_instance.rs
└── common/
    └── helpers.rs
```

**Sample Test**: `catalog_lifecycle.rs`

```rust
#[tokio::test]
async fn test_catalog_sign_verify_load() {
    // 1. Generate test keypair
    // 2. Create catalog with test decoys
    // 3. Sign with catalog-signer
    // 4. Load in stealth-browser
    // 5. Verify decoy selection works
    // 6. Test update mechanism
}
```

### 3.2 HTX Transport Validation

**New File**: `crates/htx/tests/decoy_routing.rs`

```rust
#[tokio::test]
async fn test_masked_connection_uses_decoy() {
    // 1. Start local edge-gateway as decoy
    // 2. Configure catalog pointing to it
    // 3. Dial target via HTX
    // 4. Verify connection went through decoy
    // 5. Validate TLS fingerprint match
}
```

### 3.3 DPI Validation Script

**New File**: `scripts/validate-dpi.ps1`

```powershell
# 1. Start packet capture (Wireshark/tshark)
# 2. Make masked connection to real site
# 3. Analyze captured TLS handshake
# 4. Compare fingerprint with baseline
# 5. Report pass/fail
```

---

## Phase 4: Documentation (⚠️ Medium)

### 4.1 API Documentation

**Target**: 100% rustdoc coverage for public APIs

**Template**:
```rust
/// Computes complete JA3 fingerprint from TLS parameters.
///
/// # Arguments
/// * `fp` - TLS fingerprint extracted from handshake
///
/// # Returns
/// MD5 hash of JA3 string in format:
/// `SSLVersion,Ciphers,Extensions,Curves,PointFormats`
///
/// # Example
/// ```
/// let fp = TlsFingerprint { /* ... */ };
/// let ja3 = compute_ja3_full(&fp);
/// assert_eq!(ja3.len(), 32); // MD5 hex
/// ```
pub fn compute_ja3_full(fp: &TlsFingerprint) -> String { ... }
```

### 4.2 Architecture Decision Records

**New Directory**: `docs/adr/`

```
adr/
├── 001-tls-fingerprint-mirroring.md
├── 002-htx-transport-design.md
└── 003-catalog-security-model.md
```

**Template** (ADR-001):
```markdown
# ADR-001: TLS Fingerprint Mirroring Approach

## Status: Accepted

## Context
Need to make HTX traffic indistinguishable from real HTTPS...

## Decision
Use rustls with custom ServerCertVerifier to extract...

## Consequences
- Pros: Accurate fingerprints, real handshake data
- Cons: More complex than HTTP probes

## Alternatives Considered
1. HTTP probes (rejected: inaccurate)
2. External TLS analyzer (rejected: fragile)
```

---

## Phase 5: Performance & Polish (✅ Low)

### 5.1 Criterion Benchmarks

**New Directory**: `benches/`

```
benches/
├── tls_handshake.rs
├── socks_throughput.rs
├── catalog_ops.rs
└── crypto_primitives.rs
```

**Sample**: `tls_handshake.rs`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_tls_calibration(c: &mut Criterion) {
    c.bench_function("calibrate microsoft.com", |b| {
        b.iter(|| {
            let fp = calibrate_tls_fingerprint(black_box("https://microsoft.com"));
            black_box(fp);
        });
    });
}

criterion_group!(benches, bench_tls_calibration);
criterion_main!(benches);
```

### 5.2 Error Handling Cleanup

**Migration to `thiserror`**:

```rust
// Before (anyhow - good for applications)
use anyhow::{anyhow, Result};

// After (thiserror - better for libraries)
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HtxError {
    #[error("TLS handshake failed: {0}")]
    TlsHandshake(String),
    
    #[error("Invalid catalog signature")]
    InvalidSignature,
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

### 5.3 Clippy Configuration

**Add to `Cargo.toml`**:

```toml
[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"
cargo = "warn"

# Allow some pedantic lints
too_many_arguments = "allow"
module_name_repetitions = "allow"
```

---

## Execution Order

### Week 1-2: TLS Fingerprinting
1. Create `fingerprint.rs` module
2. Implement JA3/JA4 calculation
3. Add rustls-based handshake inspection
4. Update `tls_mirror.rs`
5. Test against real sites

### Week 3: Refactoring
1. Extract constants
2. Split `main.rs` into modules
3. Verify functionality
4. Clean up dead code

### Week 4-5: Testing
1. Write integration tests
2. Add HTX transport tests
3. Create DPI validation script
4. Set up CI pipeline

### Week 6: Documentation
1. Add rustdoc to all public APIs
2. Write ADRs
3. Create deployment guide
4. Write security docs

### Week 7-8: Polish
1. Add criterion benchmarks
2. Migration to thiserror
3. Configure clippy
4. Fix all warnings
5. Final validation

---

## Success Criteria

### Phase 1 Complete When:
- [ ] JA3 hash matches real sites (tested with 10+ major domains)
- [ ] JA4 implementation verified
- [ ] TLS handshake extraction working
- [ ] No hardcoded defaults in calibration

### Phase 2 Complete When:
- [ ] All modules under 400 lines
- [ ] No magic numbers in code
- [ ] All dead code removed
- [ ] `cargo build` succeeds

### Phase 3 Complete When:
- [ ] 80%+ test coverage on new code
- [ ] All integration tests passing
- [ ] DPI validation shows indistinguishability

### Phase 4 Complete When:
- [ ] `cargo doc --no-deps` has zero warnings
- [ ] All ADRs reviewed
- [ ] Security doc peer-reviewed

### Phase 5 Complete When:
- [ ] All benchmarks in CI
- [ ] Zero clippy warnings at `warn` level
- [ ] Error types consistent

---

## Risk Mitigation

### Risk: TLS Fingerprinting Too Complex

**Mitigation**: Start with JA3 only, add JA4 if time permits

### Risk: Refactoring Breaks Functionality

**Mitigation**: Incremental changes with tests after each step

### Risk: Testing Infrastructure Takes Too Long

**Mitigation**: Prioritize integration tests, defer benchmarks if needed

---

## Dependencies

**New Crates Required**:
```toml
[dependencies]
# For TLS inspection
rustls = "0.21"
tokio-rustls = "0.24"
webpki-roots = "0.25"

# For errors
thiserror = "1.0"

# For benchmarks
[dev-dependencies]
criterion = "0.5"
```

---

## Review & Approval

Before proceeding to execution, this plan requires review for:
- Technical approach validation
- Timeline feasibility
- Resource allocation
- Priority adjustments
