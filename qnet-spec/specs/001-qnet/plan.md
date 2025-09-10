# QNet Implementation Plan

## Tech Stack Choices

### Core Languages
- **Rust**: Primary language for HTX crate, mixnode, and core networking components. Chosen for memory safety, performance, and async capabilities.
- **C**: For the C library implementation bounty. Provides wide compatibility and low-level control.
- **Go**: For the spec-compliance linter. Excellent for CLI tools and concurrent processing.

### Key Libraries & Frameworks
- **tokio**: Async runtime for Rust components, enabling high-performance concurrent operations.
- **ring**: Cryptography library for Rust, providing ChaCha20-Poly1305, Ed25519, X25519, HKDF-SHA256 implementations.
- **libp2p**: For L3 overlay mesh, handling peer discovery, multiplexing, and transport protocols.
- **rustls**: For TLS handling in HTX, with support for origin mirroring and QUIC.
- **quinn**: QUIC implementation for UDP-443 variant.
- **scion-rust**: SCION protocol implementation for L1 routing (assuming available or to be developed).
- **nym-sdk**: For L4 privacy hops integration.
- **sled**: Embedded database for local state management.
- **serde**: Serialization for CBOR and other formats.

### Alignment with Betanet 1.2

We will adopt several concrete, normative ideas inspired by Betanet v1.2 to improve interoperability and censorship-resistance:

- Deterministic CBOR encoding for all registry, key-schedule, and TemplateID contexts (Section 6.2 style).
- TemplateID-based origin calibration: implement origin template discovery and TemplateID computation (deterministic CBOR -> SHA-256) and cache policy.
- Precise L2 framing and AEAD semantics: follow the L2 frame layout (Len(u24)|Type(u8)|StreamID|AAD|Ciphertext|Tag) and KEY_UPDATE overlap semantics.
- Inner Noise XK handshake and key schedule: match the Noise XK state machine and exporter context binding to prevent downgrades.
- Translation Layer for v1.1 compatibility: provide an L2/L3 translation path to interoperate with older peers without exposing plaintext.
- Compliance Profiles (MINIMAL/STANDARD/EXTENDED) and an automated compliance test harness mirroring the v1.2 test procedures.

These choices tighten the spec for early interoperability and provide clear testable targets for the PoC.

### Infrastructure
- **Docker**: Containerization for reproducible builds and deployment.
- **Linux**: Primary target OS for servers and development.
- **GitHub Actions**: CI/CD for automated testing, fuzzing, and SLSA provenance generation.

## Architecture Overview

### Component Structure
```
qnet/
├── htx-crate/          # Rust HTX client/server (bounty)
├── mixnode/            # Rust Nym mixnode (bounty)
├── c-lib/              # C library implementation (bounty)
├── linter/             # Go spec-compliance linter (bounty)
├── utls-gen/           # uTLS template generator (bounty)
├── core/               # Shared Rust libraries
│   ├── crypto/         # ChaCha20, Ed25519, etc.
│   ├── framing/        # L2 frame handling
│   ├── routing/        # L1 SCION + HTX
│   ├── mesh/           # L3 libp2p integration
│   └── naming/         # L5 self-certifying IDs
├── cli/                # Command-line tools
├── tests/              # Unit and integration tests
└── docs/               # Specifications and guides
```

### Layer Mapping to Components
- **L0**: Handled by OS/system libraries
- **L1**: `routing/` module with SCION and HTX tunneling
- **L2**: `htx-crate/` with inner Noise XK and framing
- **L3**: `mesh/` with libp2p transports
- **L4**: `mixnode/` with Nym integration
- **L5**: `naming/` with alias ledger
- **L6**: Voucher handling in core
- **L7**: Application examples in separate repo

## Implementation Phases

### Phase 1: Core Infrastructure (2-3 months)
1. Set up Rust workspace with tokio and ring
2. Implement basic crypto primitives
3. Create L2 framing and AEAD handling
4. Develop Noise XK handshake
5. Build SCION packet structures

### Phase 2: HTX Crate (2 months)
1. TLS origin mirroring with rustls
2. QUIC support with quinn
3. Inner channel establishment
4. Frame multiplexing and flow control
5. Fuzz testing for 80% coverage

### Phase 3: Routing & Mesh (2 months)
1. SCION path construction and validation
2. HTX tunneling for non-SCION links
3. libp2p integration for peer discovery
4. Bootstrap with rotating DHT and PoW

### Phase 4: Privacy & Naming (2 months)
1. Mixnode selection with BeaconSet
2. Nym mixnet integration
3. Self-certifying ID implementation
4. Alias ledger with 2-of-3 finality

### Phase 5: Payments & Governance (1 month)
1. Voucher format and validation
2. Lightning integration
3. Node uptime scoring
4. Voting power calculations

### Phase 6: Tools & Compliance (1 month)
1. C library wrapper
2. Go linter for spec compliance
3. uTLS template generator
4. SLSA provenance in CI/CD

## Testing Strategy

### Unit Tests
- Comprehensive coverage for all crypto operations
- Frame parsing and serialization
- Handshake state machines
- Path validation logic

### Integration Tests
- End-to-end HTX connections
- Multi-hop routing scenarios
- Mixnode traffic processing
- Bootstrap discovery

### Fuzzing
- Protocol parsers with cargo-fuzz
- Crypto input validation
- Network message handling

### Compliance Testing
- Automated checks for 13 compliance points
- Origin mirroring validation
- Traffic indistinguishability tests

## Deployment & Distribution

### Containerization
- Multi-stage Docker builds for minimal images
- Separate containers for each bounty component
- Docker Compose for local development

### CI/CD Pipeline
- GitHub Actions for automated builds
- Cross-compilation for multiple architectures
- Security scanning and dependency checks
- SLSA provenance generation

### Distribution
- Pre-built binaries for Linux x86_64, ARM64
- Docker images on GitHub Container Registry
- Source code with reproducible builds

## Security Considerations

### Threat Mitigation
- Memory-safe languages (Rust, Go) for critical components
- Formal verification for crypto protocols
- Regular security audits and fuzzing
- Post-quantum crypto integration

### Privacy Protection
- Traffic analysis resistance through cover behavior
- Metadata minimization
- Anti-correlation measures
- Jurisdictionally diverse mixnode selection

## Performance Targets

### Benchmarks
- HTX throughput: 10 Gbps+ on 10G NIC
- Mixnode processing: 25k packets/sec on 4-core VPS
- Bootstrap time: < 30 seconds
- Path switch time: < 300ms

See Phase 6 Task T6.6 for detailed micro-benchmarks, zero-copy refactors, QUIC integration toggles, and CI guardrails (nightly perf job, regression thresholds).

### Resource Requirements
- Minimum: 1 CPU core, 512MB RAM for client
- Recommended: 4+ cores, 4GB RAM for nodes
- Storage: 10GB for ledger and state

## Risk Assessment

### Technical Risks
- SCION implementation complexity
- PQ crypto integration timeline
- Interoperability with existing networks

### Mitigation
- Incremental development with working prototypes
- Extensive testing and fuzzing
- Community collaboration on bounties

## Success Metrics

- All 13 compliance points implemented
- Bounty deliverables completed
- End-to-end connectivity demonstrated
- Performance targets met
- Security audit passed

## Repository and Ecosystem Management

### Dual Audience Strategy
QNet serves both developers (toolkit/framework users) and end users (ready-to-use applications). To balance this:
- **Developer Focus**: Core crates in `crates/` and examples in `examples/` for integration (e.g., HTX crate for tunneling).
- **User Focus**: Applications in `apps/` like the stealth browser for easy anonymous browsing.
- **Documentation**: Separate guides—technical docs for devs, quick starts for users.

### Repository Organization
- Maintain modular structure: `crates/` for toolkit, `apps/` for user apps.
- Enforce `.gitignore` for build artifacts to manage 5GB+ size.
- Use Git LFS for large assets if needed.

### CI/CD Pipelines
- **Toolkit Pipeline**: Fast Rust builds/tests for `crates/` (unit tests, fuzzing).
- **Apps Pipeline**: Includes browser packaging (MSI/APK/DMG), integration tests, and pre-built binaries.
- Separate workflows: Toolkit on PRs, apps on releases for efficiency.

### Long-Term Ecosystem Growth
- Encourage third-party apps via modular crates.
- Provide pre-built binaries via GitHub Releases.
- Plan for separate repos if user apps grow large.

## Research & Dependencies

### External Dependencies
- tokio: Async runtime
- ring: Crypto primitives
- libp2p: P2P networking
- rustls: TLS implementation
- quinn: QUIC protocol
- sled: Database
- serde: Serialization

### Research Areas
- SCION Rust implementation status
- Nym SDK integration
- PQ crypto library maturity
- Cover traffic generation techniques

This plan provides a modular, bounty-friendly architecture that can be developed incrementally while maintaining compliance with the QNet specification.
