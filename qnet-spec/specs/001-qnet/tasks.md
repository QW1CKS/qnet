# QNet Unified Implementation Tasks

This is the single source of truth for QNet development.

## 🏁 Phase 1: Core Infrastructure (Done)
*Foundational networking and cryptography layers.*

- [x] **Project Setup**: Workspace, crates structure, CI/CD.
- [x] **Crypto Primitives (`core-crypto`)**: ChaCha20-Poly1305, Ed25519, X25519, HKDF-SHA256.
- [x] **L2 Framing (`core-framing`)**: AEAD-protected frames, padding, stream multiplexing.
- [x] **HTX Transport (`htx`)**:
    - [x] TLS Origin Mirroring (ClientHello cloning).
    - [x] Inner Noise XK Handshake.
    - [x] Traffic Disguise (Jitter, Padding).
- [x] **Catalog System**:
    - [x] Signed Catalog Schema (DET-CBOR).
    - [x] Catalog Signer Tool.
    - [x] Secure Loader & Verifier.

## 🚧 Phase 2: The "Helper Node" (Current Focus)
*Turning the local Helper into a full P2P mesh node.*

- [ ] **L3 P2P Mesh (`core-mesh`)**:
    - [x] Basic libp2p integration.
    - [ ] **Peer Discovery**: Find other QNet nodes (DHT/Rendezvous).
    - [ ] **Relay Logic**: Ability to forward traffic for other users.
    - [ ] **Circuit Building**: Construct multi-hop paths (Client -> Peer -> Peer -> Exit).
- [ ] **Helper Service (`stealth-browser` binary)**:
    - [x] SOCKS5 Proxy Interface.
    - [x] Catalog Integration.
    - [ ] **Mesh Integration**: Hook SOCKS5 requests into the L3 Mesh instead of direct HTX.
    - [ ] **Auto-Update**: Self-updating mechanism for the binary.

## 🚧 Phase 3: User Experience (Current Focus)
*Delivering the network to end users.*

- [ ] **Browser Extension**:
    - [ ] **UI**: Connect/Disconnect toggle, Status dashboard.
    - [ ] **Native Messaging**: Secure communication with Helper.
    - [ ] **Proxy Management**: Auto-configure browser proxy settings.
- [ ] **Installers**:
    - [ ] **Windows**: MSI installer bundling Helper + Extension link.
    - [ ] **Linux/macOS**: Scripts/Packages.

## 🔮 Phase 4: Advanced Privacy (Future)
*Enhancing anonymity and resistance.*

- [ ] **L4 Mixnet**: Nym integration for high-latency, high-anonymity traffic.
- [ ] **L5 Naming**: Self-certifying IDs (Petnames) to replace DNS.
- [ ] **L6 Payments**: Voucher/Cashu system for incentivizing relays.
- [ ] **L7 Governance**: Decentralized protocol upgrades.

## 🛠️ Tools & Compliance
- [x] **Spec Linter**: Go tool for validating compliance.
- [x] **uTLS Generator**: Tool to keep TLS fingerprints up to date.
- [x] **Performance Benchmarks**: CI jobs for throughput/latency.
