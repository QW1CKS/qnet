# QNet Specification

## Overview
QNet is a decentralized, censorship-resistant network designed to replace the public Internet. It provides strong user privacy, decentralized operation, and resistance to censorship through layered architecture and advanced cryptography.

## User Stories

### Core Network Functionality
- **As a user**, I want to connect to QNet using standard internet access so that I can access decentralized services without relying on centralized infrastructure.
- **As a developer**, I want to build applications on QNet using self-certifying identities so that users can verify service authenticity without external authorities.
- **As a privacy-conscious user**, I want my traffic to be routed through mixnodes so that observers cannot correlate my communications.
- **As a developer**, I want to integrate QNet's protocol stack into my apps via modular crates (e.g., HTX for tunneling) so that I can build custom privacy tools without reinventing cryptography.
- **As an end user**, I want a ready-to-use stealth browser that mimics normal HTTPS traffic so that I can browse anonymously without ISP tracking or technical setup.

### Layer Architecture
- **L0 Access Media**: Support any IP bearer (fibre, 5G, satellite, LoRa, etc.)
- **L1 Path Selection & Routing**: Use SCION + HTX-tunnelled transitions for secure routing
- **L2 Cover Transport**: HTX over TCP-443/QUIC-443 with origin-mirrored TLS
- **L3 Overlay Mesh**: libp2p-v2 object relay for peer-to-peer connections
- **L4 Privacy Hops**: Nym mixnet for optional anonymity
- **L5 Naming & Trust**: Self-certifying IDs + 3-chain alias ledger
- **L6 Payments**: Federated Cashu + Lightning for micro-payments
- **L7 Applications**: Web-layer replacement services

### Cryptography & Security
- **As a security engineer**, I want all communications encrypted with ChaCha20-Poly1305, Ed25519 signatures, X25519 DH, and HKDF-SHA256 so that data is protected against eavesdropping and tampering.
- **As a future-proof system**, I want post-quantum hybrid X25519-Kyber768 from 2027 so that the network remains secure against quantum attacks.

### Key Features
- Origin-mirrored TLS handshake with auto-calibration
- Noise XK inner handshake for mutual authentication
- SCION packet headers for path validation
- Mixnode selection using BeaconSet randomness
- Alias ledger with 2-of-3 finality
- Voucher-based payments
- Anti-correlation fallback with cover connections

### Improvements Over Betanet
- Enhanced anti-correlation measures
- Adaptive Proof-of-Work for bootstrap
- Better scalability and user adoption incentives
- Reproducible builds with SLSA provenance

## Functional Requirements

### Networking
1. Clients MUST mirror front origin TLS fingerprints exactly
2. Inner channel MUST use Noise XK with PQ hybrid from 2027
3. Paths MUST be validated using SCION signatures
4. Bootstrap MUST use rotating DHT with adaptive PoW
5. Mixnodes MUST be selected deterministically with diversity

### Privacy & Security
1. Traffic MUST be indistinguishable from normal HTTPS
2. All frames MUST be AEAD-protected
3. Replay protection MUST use per-direction counters
4. Congestion feedback MUST influence path selection
5. Emergency Advance MUST be available for naming liveness

### Payments & Governance
1. Vouchers MUST be 128-byte with aggregated signatures
2. Voting power MUST cap per-AS and per-org
3. Quorum MUST require 2/3 diversity
4. Upgrades MUST wait 30 days after threshold

### Compliance
1. Implementations MUST pass all 13 compliance checks
2. Builds MUST be reproducible
3. Binaries MUST have SLSA 3 provenance

## Bounties
- HTX client/server crate
- High-performance Nym mixnode
- C library implementation
- Spec-compliance linter
- Chrome-Stable uTLS generator

## Acceptance Criteria
- All user stories implemented
- Functional requirements met
- Compliance tests pass
- Bounties deliverable
- Better than Betanet in key areas

## Review & Acceptance Checklist
- [ ] Spec covers all layers L0-L7
- [ ] Cryptography requirements specified
- [ ] Privacy features detailed
- [ ] Governance and payments included
- [ ] Improvements over Betanet identified
- [ ] Bounties clearly defined
- [ ] Compliance points enumerated
