# QNet Production Hardening Tasks

## 🔴 High Priority (Blocking Production)

### 1. Fix TLS Fingerprint Mirroring
- [ ] Implement complete JA3 calculation (version + ciphers + extensions + groups + ec_point_formats)
- [ ] Implement JA4 calculation for enhanced fingerprinting
- [ ] Replace reqwest HTTP probes with actual TLS handshake inspection
- [ ] Create custom rustls verifier to extract real TLS parameters
- [ ] Add TLS fingerprint extraction from live connections
- [ ] Test fingerprint accuracy against real sites (Microsoft, Google, Cloudflare)
- [ ] Update `htx/tls_mirror.rs` to use complete fingerprints

### 2. Harden HTX Transport
- [ ] Add integration test: catalog → decoy selection → masked connection
- [ ] Add integration test: SOCKS5 → HTX → verify decoy used
- [ ] Create Wireshark/Tshark validation script
- [ ] Test indistinguishability with DPI tools
- [ ] Add replay attack prevention tests
- [ ] Document HTX security guarantees
- [ ] Add fuzzing for HTX frame parsing

### 3. Refactor stealth-browser
- [ ] Split main.rs into modules:
  - [ ] `status.rs` - status server and endpoints
  - [ ] `socks.rs` - SOCKS5 proxy handler
  - [ ] `catalog.rs` - catalog management
  - [ ] `config.rs` - configuration and CLI parsing
  - [ ] `lib.rs` - core application state
- [ ] Extract magic numbers to constants module
- [ ] Remove all commented-out dead code
- [ ] Clean up `#[allow(dead_code)]` attributes
- [ ] Add module-level documentation
- [ ] Verify all functionality after refactor

## ⚠️ Medium Priority (Nice-to-Have)

### 4. End-to-End Testing
- [ ] Test: Catalog signing → verification → loading → persistence
- [ ] Test: Decoy catalog → weighted selection → rotation
- [ ] Test: SOCKS5 request → masked mode → HTX dial → response
- [ ] Test: Status API endpoints with real HTTP clients
- [ ] Test: Catalog update cycle (mirrors, verification, atomic swap)
- [ ] Test: Single-instance enforcement
- [ ] Add CI pipeline for E2E tests

### 5. Documentation Audit
- [ ] Add rustdoc for all public functions in `core-crypto`
- [ ] Add rustdoc for all public functions in `htx`
- [ ] Add rustdoc for `catalog-signer` API
- [ ] Create ADR-001: TLS Fingerprint Mirroring Approach
- [ ] Create ADR-002: HTX vs Standard VPN Protocols
- [ ] Create ADR-003: Catalog Distribution Security Model
- [ ] Write security considerations document
- [ ] Document threat model and mitigations
- [ ] Create deployment guide
- [ ] Write troubleshooting guide

### 6. Performance Benchmarks
- [ ] Benchmark: TLS handshake latency (direct vs HTX)
- [ ] Benchmark: SOCKS5 throughput (various payload sizes)
- [ ] Benchmark: Catalog loading and verification time
- [ ] Benchmark: Decoy selection performance (1k+ entries)
- [ ] Benchmark: AEAD encryption/decryption throughput
- [ ] Benchmark: Ed25519 signature verification
- [ ] Add criterion benchmarks to CI
- [ ] Create performance regression tracking

## ✅ Low Priority (Polish)

### 7. Code Cleanup
- [ ] Remove all `#[allow(dead_code)]` or justify retention
- [ ] Replace `anyhow` with `thiserror` for library errors
- [ ] Add `clippy` lints to workspace Cargo.toml
- [ ] Fix all clippy warnings at `warn` level
- [ ] Standardize error types across crates
- [ ] Add `rustfmt` configuration
- [ ] Enforce formatting in CI
- [ ] Add pre-commit hooks

## Progress Tracking

**High Priority**: 0/21 complete (0%)
**Medium Priority**: 0/26 complete (0%)
**Low Priority**: 0/8 complete (0%)

**Overall**: 0/55 tasks complete (0%)
