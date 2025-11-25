# QNet Protocol Specification v1.0

## 1. Overview
QNet is a **decentralized overlay network** designed to provide censorship resistance and privacy by disguising all traffic as legitimate HTTPS connections to popular domains (decoys).

Unlike traditional VPNs, QNet uses a **Peer-to-Peer (P2P) mesh** where every user node (Helper) can act as a relay, creating a resilient network that is mathematically difficult to block without breaking the internet.

## 2. Architecture

### 2.1 The "Helper Node" Model
The core unit of QNet is the **Helper Node**.
- **Function**: A local background service that acts as a full network peer.
- **Role**:
    - **Client**: Accepts SOCKS5 traffic from the user's browser.
    - **Peer**: Routes traffic through the QNet mesh.
    - **Exit**: Fetches content from the destination (if configured).
- **Deployment**: Distributed as a small binary (`stealth-browser`) bundled with a Browser Extension for UI control.

### 2.2 Layered Stack
QNet implements a 7-layer protocol stack:

| Layer | Name | Function | Implementation |
|-------|------|----------|----------------|
| **L7** | **Application** | Browser Extension, SOCKS5 Interface | `apps/stealth-browser` |
| **L6** | **Incentive** | Payments & Reputation (Future) | Vouchers / Cashu |
| **L5** | **Naming** | Decentralized DNS (Future) | Self-Certifying IDs |
| **L4** | **Privacy** | Anonymity Mixing (Optional) | Nym Mixnet |
| **L3** | **Mesh** | P2P Routing & Discovery | `core-mesh` (libp2p) |
| **L2** | **Transport** | **HTX**: Disguised Secure Channel | `htx` (TLS Mirroring) |
| **L1** | **Path** | Path Selection & Validation | SCION / IP |
| **L0** | **Physical** | Underlying IP Network | TCP / UDP / QUIC |

## 3. Core Protocols

### 3.1 L2: HTX (Hypertext Transport Extension)
The foundation of QNet's censorship resistance.
- **Goal**: Make QNet traffic indistinguishable from HTTPS to a decoy site.
- **Mechanism**:
    1.  **TLS Mirroring**: Handshake exactly mimics the decoy's fingerprint (JA3, ALPN, Extensions).
    2.  **Inner Handshake**: Establishes a Noise XK secure channel *inside* the TLS stream.
    3.  **Traffic Shaping**: Padding and timing jitter to match decoy behavior.
- **Requirement**: ISP sees `HTTPS -> microsoft.com`; Reality is `HTX -> QNet Node`.

### 3.2 L3: Overlay Mesh
The routing layer that bypasses IP blocks.
- **Discovery**: Nodes find each other via DHT or Rendezvous points (bootstrapped via signed catalogs).
- **Routing**:
    - **Fast Mode (1-Hop)**: User -> Peer -> Destination.
    - **Private Mode (3-Hop)**: User -> Peer -> Peer -> Peer -> Destination.
- **Resilience**: If direct access to a site is blocked, the mesh routes around the block via peers in free jurisdictions.

## 4. Cryptography Standards
All implementations MUST adhere to these primitives:
- **Cipher**: ChaCha20-Poly1305 (AEAD).
- **Key Exchange**: X25519 (ECDH).
- **Signatures**: Ed25519.
- **Key Derivation**: HKDF-SHA256.
- **Post-Quantum**: Hybrid X25519-Kyber768 (Planned for 2027).

## 5. Configuration & Trust
- **Catalogs**: Configuration (decoys, seeds, updates) is distributed via **Signed Catalogs**.
- **Updates**: Nodes fetch updates from redundant mirrors (GitHub, CDNs), verifying the detached Ed25519 signature before applying.
- **Trust**: No central authority. Trust is anchored in the cryptographic identity of peers and the signed catalog public keys.
