# QNet Protocol Specification v1.0

## 1. Overview
QNet is a **decentralized overlay network** designed to provide censorship resistance and privacy through traffic obfuscation and P2P mesh routing.

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
| **L2** | **Transport** | **HTX**: Obfuscated Secure Channel | `htx` (TLS Fingerprint Resistance) |
| **L1** | **Path** | Path Selection & Validation | SCION / IP |
| **L0** | **Physical** | Underlying IP Network | TCP / UDP / QUIC |

## 3. Core Protocols

### 3.1 L2: HTX (Hypertext Transport Extension)
The foundation of QNet's censorship resistance.
- **Goal**: Make QNet traffic resist ML-based fingerprinting and protocol analysis.
- **Mechanism**:
    1.  **TLS Fingerprint Resistance**: Handshake uses common browser fingerprints (JA3, ALPN, Extensions).
    2.  **Inner Handshake**: Establishes a Noise XK secure channel *inside* the TLS stream.
    3.  **Traffic Shaping**: Padding and timing jitter to resist traffic analysis.
- **Result**: Traffic appears as standard HTTPS, difficult to distinguish from normal browsing.

### 3.2 L3: Overlay Mesh
The routing layer that bypasses IP blocks.
- **Discovery**: Nodes find each other via operator directory HTTP queries or mDNS (bootstrapped via hardcoded operators).
- **Routing**:
    - **Fast Mode (1-Hop)**: User -> Peer -> Destination.
    - **Private Mode (3-Hop)**: User -> Peer -> Peer -> Peer -> Destination.
- **Resilience**: If direct access to a site is blocked, the mesh routes around the block via peers in free jurisdictions.

### 3.3 Bootstrap Strategy
**Decentralized Peer Discovery**

QNet uses a hybrid bootstrap approach that eliminates central points of failure:

#### Primary: Operator Directory
- HTTP-based peer registry maintained by operator nodes
- Relay nodes register via heartbeat (POST /api/relay/register every 30s)
- Client nodes query directory on startup (GET /api/relays/by-country)
- Lightweight JSON API (no blockchain/DHT complexity)

#### Fallback: Hardcoded Operators
- 6 hardcoded operator bootstrap nodes (DigitalOcean droplets across regions)
- Used if directory query fails or returns no peers
- Always available for initial mesh entry

**Result**: Zero single point of failure - network remains accessible with fallback mechanisms.

### 3.4 Exit Node Architecture
**Professional Exit Nodes for User Safety**

QNet employs a three-tier model to protect users from legal liability:

#### Tier 1: User Helpers (Relay-Only Mode)
- **Default configuration**: Users run as relay nodes only
- **Function**: Forward encrypted packets through the mesh
- **Legal Protection**: Cannot see packet contents (end-to-end encrypted)
- **No Risk**: Never make actual web requests, only relay encrypted traffic
- **99% of network**: Most users operate in this safe mode

#### Tier 2: Operator Exit Nodes (Primary Exits)
- **DigitalOcean droplets** ($8-18/month total for global coverage)
- **Function**: Decrypt packets and make actual web requests
- **Professional Operation**: Proper logging, abuse policies, legal notices
- **Reliability**: 99.9% uptime, fast bandwidth, multiple regions
- **User Protection**: Home users never exposed to exit node legal risks

#### Tier 3: Volunteer Exits (Optional)
- **Opt-in only**: Experienced users can choose to act as exits
- **Clear Warnings**: Legal liability disclosures before enabling
- **Exit Policies**: Granular control over what traffic to exit
- **Recommended**: Run on VPS, not home connections

**Cost Efficiency**: 
- 2 droplets @ $4/month = $8/month (minimal deployment)
- 3 droplets @ $6/month = $18/month (recommended global coverage)
- Serves 200-400 users per region with basic droplets

**Design Philosophy**: Users relay safely, operators handle exit risks professionally.

## 4. Cryptography Standards
All implementations MUST adhere to these primitives:
- **Cipher**: ChaCha20-Poly1305 (AEAD).
- **Key Exchange**: X25519 (ECDH).
- **Signatures**: Ed25519.
- **Key Derivation**: HKDF-SHA256.
- **Post-Quantum**: Hybrid X25519-Kyber768 (Planned for 2027).

## 5. Configuration & Trust
- **Bootstrap**: Initial peers discovered via operator directory with hardcoded fallback operators.
- **Updates**: Nodes fetch updates from redundant mirrors (GitHub, CDNs), verifying the detached Ed25519 signature before applying.
- **Trust**: No central authority. Trust is anchored in the cryptographic identity of peers.
