# QNet Implementation Tasks

## Task Overview
This task list breaks down the QNet implementation plan into specific, actionable items. Tasks are prioritized as High (critical for PoC), Medium (important for functionality), or Low (nice-to-have). Time estimates are in person-days. Dependencies are noted where applicable.

## Phase 1: Core Infrastructure Setup (Priority: High)

### T1.1: Project Structure Setup
**Description**: Set up Rust workspace with tokio, ring, and basic project structure. Create core modules for crypto, framing, and common utilities.
**Priority**: High
**Dependencies**: None
**Time Estimate**: 3 days
**Assignee**: Core Developer
**Notes**: Include Docker setup for reproducible builds

### T1.2: Crypto Primitives Implementation
**Description**: Implement ChaCha20-Poly1305, Ed25519, X25519, HKDF-SHA256 using ring crate. Add deterministic CBOR encoding.
**Priority**: High
**Dependencies**: T1.1
**Time Estimate**: 5 days
**Assignee**: Crypto Specialist
**Notes**: Include unit tests and fuzzing setup

### T1.3: L2 Frame Handling
**Description**: Implement L2 frame types (STREAM, WINDOW_UPDATE, PING, KEY_UPDATE, CLOSE) with AEAD encryption/decryption.
**Priority**: High
**Dependencies**: T1.2
**Time Estimate**: 4 days
**Assignee**: Networking Developer
**Notes**: Focus on frame parsing and serialization

### T1.4: Noise XK Handshake
**Description**: Implement complete Noise XK handshake with state machine, key derivation, and post-quantum hybrid support.
**Priority**: High
**Dependencies**: T1.2
**Time Estimate**: 6 days
**Assignee**: Crypto Specialist
**Notes**: Include test vectors from spec
   
### T1.5: Deterministic CBOR & TemplateID
**Description**: Implement deterministic CBOR encoder/decoder and TemplateID computation (DET-CBOR -> SHA-256) used for origin templates and exporter contexts.
**Priority**: High
**Dependencies**: T1.2
**Time Estimate**: 4 days
**Assignee**: Serialization Developer
**Notes**: Include canonical ordering rules, no indefinite-length items, and test vectors.

## Phase 2: HTX Crate PoC (Priority: High)

### T2.1: TLS Origin Mirroring
**Description**: Implement origin template discovery, ClientHello construction, and fingerprint matching using rustls.
**Priority**: High
**Dependencies**: T1.1, T1.4
**Time Estimate**: 7 days
**Assignee**: TLS Developer
**Notes**: Support both TCP and QUIC variants

### T2.2: Inner Channel Establishment
**Description**: Integrate Noise XK into TLS/QUIC connection, handle key schedule and exporter context.
**Priority**: High
**Dependencies**: T1.4, T2.1
**Time Estimate**: 5 days
**Assignee**: Networking Developer
**Notes**: Ensure version binding for downgrade resistance

### T2.3: Frame Multiplexing
**Description**: Implement stream ID allocation, flow control (WINDOW_UPDATE), and concurrent frame processing.
**Priority**: High
**Dependencies**: T1.3, T2.2
**Time Estimate**: 4 days
**Assignee**: Networking Developer
**Notes**: Support both client and server roles

### T2.4: HTX Crate API
**Description**: Create dial() and accept() APIs for HTX connections, with multiplexed stream() support.
**Priority**: High
**Dependencies**: T2.3
**Time Estimate**: 3 days
**Assignee**: API Developer
**Notes**: Match bounty requirements for reusability

### T2.5: Fuzzing and Testing
**Description**: Set up cargo-fuzz for 80% line/branch coverage, add integration tests for end-to-end connections.
**Priority**: High
**Dependencies**: T2.4
**Time Estimate**: 5 days
**Assignee**: QA Engineer
**Notes**: Include ECH stub and anti-correlation fallback

### T2.6: L2 Frame Semantics & KEY_UPDATE Behavior
**Description**: Implement exact L2 frame layout and AEAD AAD semantics, including KEY_UPDATE concurrency rules with 3-frame overlap acceptance and nonce/nonce-salt derivation.
**Priority**: High
**Dependencies**: T1.3, T1.4
**Time Estimate**: 3 days
**Assignee**: Networking Developer
**Notes**: Add unit tests and integration checks for key rotation edge cases.

## Phase 3: Routing & Mesh (Priority: Medium)

### T3.1: SCION Packet Structures
**Description**: Implement SCION packet header, path segments, and signature validation.
**Priority**: Medium
**Dependencies**: T1.1
**Time Estimate**: 6 days
**Assignee**: Routing Developer
**Notes**: Research existing Rust SCION implementations

### T3.2: HTX Tunneling
**Description**: Implement HTX-tunnelled transitions for non-SCION links with control streams.
**Priority**: Medium
**Dependencies**: T2.4, T3.1
**Time Estimate**: 4 days
**Assignee**: Networking Developer
**Notes**: Handle gateway bridging and replay protection

### T3.3: libp2p Integration
**Description**: Set up libp2p for peer discovery, multiplexing, and transport protocols (/betanet/htx/1.1.0).
**Priority**: Medium
**Dependencies**: T2.4
**Time Estimate**: 5 days
**Assignee**: P2P Developer
**Notes**: Include capability exchange and handshake

### T3.4: Bootstrap Discovery
**Description**: Implement rotating DHT, mDNS, Bluetooth LE, and DNS fallback with adaptive PoW.
**Priority**: Medium
**Dependencies**: T3.3
**Time Estimate**: 4 days
**Assignee**: Discovery Developer
**Notes**: Use BeaconSet for deterministic seeds

### T3.5: Translation Layer (v1.1 Interop)
**Description**: Build an L2/L3 translation layer to interoperate with legacy v1.1 peers, including mapping rules, synthesized KEY_UPDATE behavior, and exporter context binding.
**Priority**: Medium
**Dependencies**: T2.6, T3.1
**Time Estimate**: 4 days
**Assignee**: Interop Developer
**Notes**: Ensure TL never exposes plaintext beyond node boundary.

## Phase 4: Privacy & Naming (Priority: Medium)

### T4.1: Mixnode Selection
**Description**: Implement BeaconSet generation, VRF-based hop selection, and diversity requirements.
**Priority**: Medium
**Dependencies**: T1.2
**Time Estimate**: 5 days
**Assignee**: Privacy Developer
**Notes**: Integrate with Nym SDK

### T4.2: Nym Mixnet Integration
**Description**: Add Sphinx packet processing, cover traffic, and rate limiting for 25k pkt/s performance.
**Priority**: Medium
**Dependencies**: T4.1
**Time Estimate**: 7 days
**Assignee**: Mixnet Developer
**Notes**: Focus on high-performance Rust implementation

### T4.3: Self-Certifying IDs
**Description**: Implement PeerID generation using multihash(SHA-256(pubkey)) and name resolution.
**Priority**: Medium
**Dependencies**: T1.2
**Time Estimate**: 3 days
**Assignee**: Identity Developer
**Notes**: Support both hex and Base32 encoding

### T4.4: Alias Ledger
**Description**: Build 3-chain alias ledger with 2-of-3 finality, Emergency Advance, and quorum certificates.
**Priority**: Medium
**Dependencies**: T4.3
**Time Estimate**: 6 days
**Assignee**: Ledger Developer
**Notes**: Integrate with Handshake, Filecoin, Ethereum L2

## Phase 5: Payments & Governance (Priority: Low)

### T5.1: Voucher System
**Description**: Implement 128-byte voucher format with FROST-Ed25519 mints and aggregated signatures.
**Priority**: Low
**Dependencies**: T1.2
**Time Estimate**: 4 days
**Assignee**: Payments Developer
**Notes**: Support Lightning settlement

### T5.2: Governance Scoring
**Description**: Add node uptime scoring, voting power calculations with AS/org caps.
**Priority**: Low
**Dependencies**: T3.3
**Time Estimate**: 3 days
**Assignee**: Governance Developer
**Notes**: Implement 30-day upgrade delays

## Phase 6: Tools & Compliance (Priority: Medium)

### T6.1: C Library Implementation
**Description**: Create C library wrapper for HTX with full API compatibility.
**Priority**: Medium
**Dependencies**: T2.4
**Time Estimate**: 5 days
**Assignee**: C Developer
**Notes**: Focus on cross-platform compatibility

### T6.2: Go Spec Linter
**Description**: Build CLI tool to check 11 compliance points, generate SBOM, and GitHub Action template.
**Priority**: Medium
**Dependencies**: All previous
**Time Estimate**: 4 days
**Assignee**: Tooling Developer
**Notes**: Include automated test procedures

### T6.3: uTLS Template Generator
**Description**: Create utility for deterministic ClientHello blobs with JA3 self-test and auto-refresh.
**Priority**: Medium
**Dependencies**: T2.1
**Time Estimate**: 3 days
**Assignee**: Tooling Developer
**Notes**: Support Chrome Stable N-2

### T6.4: SLSA Provenance
**Description**: Set up GitHub Actions for reproducible builds and SLSA 3 provenance artifacts.
**Priority**: Medium
**Dependencies**: All previous
**Time Estimate**: 2 days
**Assignee**: DevOps Engineer
**Notes**: Include multi-architecture builds

### T6.5: Compliance Test Harness
**Description**: Implement an automated test harness that runs the v1.2-style test procedures (crypto/frame/key-update, discovery, routing, relays, BN-Ticket header checks) and emits pass/fail per compliance profile.
**Priority**: High
**Dependencies**: T2.5, T2.6, T3.3
**Time Estimate**: 6 days
**Assignee**: QA Engineer
**Notes**: Integrate into CI and produce machine-readable reports for the linter.

## Task Dependencies Graph

```
T1.1
├── T1.2
│   ├── T1.3
│   │   └── T1.4
│   │       ├── T2.1
│   │       │   ├── T2.2
│   │       │   │   ├── T2.3
│   │       │   │   │   └── T2.4
│   │       │   │   │       └── T2.5
│   │       │   │   │           └── T6.1
│   │       │   │   └── T3.3
│   │       │   │       ├── T3.4
│   │       │   │       └── T4.1
│   │       │   │           └── T4.2
│   │       │   └── T6.3
│   │       └── T4.3
│   │           └── T4.4
│   └── T3.1
│       └── T3.2
└── T5.1
    └── T5.2
        └── T6.2
            └── T6.4
```

## Milestones

- **M1 (Month 1)**: Core infrastructure complete (T1.1-T1.4)
- **M2 (Month 2)**: HTX PoC working (T2.1-T2.5)
- **M3 (Month 3)**: Basic routing and mesh (T3.1-T3.4)
- **M4 (Month 4)**: Privacy features (T4.1-T4.4)
- **M5 (Month 5)**: Tools and compliance (T6.1-T6.4)
- **M6 (Month 6)**: Payments and governance (T5.1-T5.2)

## Risk Mitigation

- **Parallel Development**: High-priority tasks can be developed in parallel where dependencies allow
- **Incremental Testing**: Each task includes unit tests to catch issues early
- **Modular Design**: Components designed for independent development and bounty submission
- **Research Tasks**: Allocate time for investigating complex areas like SCION and PQ crypto

## Success Criteria

- All High priority tasks completed for PoC
- HTX crate meets bounty requirements
- End-to-end connectivity demonstrated
- Compliance tests passing
- Modular components ready for bounties
