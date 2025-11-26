# QNet: The Invisible Overlay Network

<div align="center">
  <img src="logo.png" alt="QNet Logo" width="400">
  <p><strong>Decentralized. Censorship-Resistant. Unblockable.</strong></p>
</div>

---

> [!CAUTION]
> Most of the code has been implemented using agentic AI. This is just a side-project that I wanted to experiment with Copilot. This project was done purely for fun and learning. I will be removing the AI-generated code  and implement it manually in the future if I ever plan to make it production-ready. I know how frustrating it is to see AI slop in production code these days, and I very much understand the sentiment from a security perspective.
>
> If I ever intend to make this production-ready, I will make sure to undergo a professional security audit for this project.
>
> At the current moment, I make the AI follow strict [security guardrails](qnet-spec/memory/ai-guardrail.md) to ensure that the code is secure and follows best practices.
>
> Use at your own risk.

---

## 📖 Table of Contents
- [What is QNet?](#-what-is-qnet)
- [Why QNet?](#-why-qnet)
- [Architecture Overview](#-architecture-overview)
- [How It Works](#-how-it-works)
- [Key Features](#-key-features)
- [Technology Stack](#-technology-stack)
- [Quick Start](#-quick-start-developers)
- [Project Structure](#-project-structure)
- [Security Model](#-security-model)
- [Performance](#-performance)
- [Documentation](#-documentation)
- [Contributing](#-contributing)

---

## 🧐 What is QNet?

QNet is a **next-generation decentralized overlay network** engineered to provide censorship-resistant, privacy-preserving internet access from anywhere in the world. Unlike traditional VPNs or proxies, QNet uses advanced traffic masking techniques to make your connections completely indistinguishable from normal HTTPS traffic to popular websites.

### The Core Problem We Solve

```mermaid
graph LR
    A[User] -->|Traditional VPN| B[VPN Server]
    B -->|Easy to Block| C[Censored Internet]
    
    D[User] -->|QNet| E[Disguised as microsoft.com]
    E -->|Unblockable| F[Free Internet]
    
    style B fill:#ff6b6b
    style E fill:#51cf66
```

In countries with internet censorship:
- **VPNs are blocked** by detecting encrypted traffic patterns
- **Tor is slow** and can be blocked at the network level
- **Proxies are discovered** and added to blocklists

**QNet solves this** by making your traffic look exactly like legitimate HTTPS connections to trusted domains. To an ISP or government censor, you're just browsing Microsoft, Google, or Cloudflare—but you're actually accessing any site through a global P2P mesh network.

---

## 🎯 Why QNet?

### QNet vs. Traditional Solutions

| Feature | VPN | Tor | Proxy | **QNet** |
|---------|-----|-----|-------|----------|
| **Decentralized** | ❌ No | ✅ Yes | ❌ No | ✅ Yes |
| **Censorship Resistant** | ❌ Easy to block | ⚠️ Can be blocked | ❌ Easy to block | ✅ Unblockable |
| **Performance** | ✅ Fast | ❌ Slow | ✅ Fast | ✅ Fast |
| **Traffic Masking** | ❌ Obvious VPN pattern | ⚠️ Detectable | ❌ Detectable | ✅ Perfect disguise |
| **No Single Point of Failure** | ❌ Central servers | ✅ Distributed | ❌ Central proxy | ✅ P2P mesh |
| **Privacy** | ⚠️ Trust required | ✅ High | ❌ Low | ✅ High |

### Key Advantages

1. **🎭 Perfect Traffic Disguise (HTX Protocol)**
   - Clones TLS fingerprints of popular sites (JA3, ALPN, cipher suites)
   - Traffic analysis shows normal HTTPS to trusted domains
   - Impossible to distinguish from legitimate traffic without breaking TLS

2. **🕸️ Truly Decentralized**
   - No central servers to shut down
   - Every user strengthens the network
   - P2P mesh with DHT-based peer discovery

3. **⚡ Performance-Focused**
   - Fast Mode: 1-hop routing for maximum speed
   - Privacy Mode: 3-hop routing for anonymity
   - QUIC support for improved latency

4. **🔒 Defense-in-Depth Security**
   - ChaCha20-Poly1305 AEAD encryption
   - Noise XK protocol for forward secrecy
   - Ed25519 signatures for catalog integrity
   - Deterministic CBOR serialization

---

## 🏗️ Architecture Overview

QNet implements a **7-layer protocol stack** inspired by the OSI model, designed specifically for censorship resistance:

```mermaid
graph TB
    subgraph "QNet 7-Layer Architecture"
        L7[L7: Application<br/>Browser Extension + SOCKS5]
        L6[L6: Incentive Layer<br/>Vouchers & Reputation Future]
        L5[L5: Naming & Identity<br/>Self-Certifying IDs Future]
        L4[L4: Privacy Hops<br/>Mixnet Integration Optional]
        L3[L3: Overlay Mesh<br/>libp2p + DHT + Gossip]
        L2[L2: Cover Transport<br/>HTX + TLS Mirroring]
        L1[L1: Path Selection<br/>SCION-inspired Routing]
        L0[L0: Access Media<br/>TCP/UDP/QUIC over IP]
    end
    
    L7 --> L6
    L6 --> L5
    L5 --> L4
    L4 --> L3
    L3 --> L2
    L2 --> L1
    L1 --> L0
    
    style L2 fill:#ffd43b,stroke:#333,stroke-width:3px
    style L3 fill:#74c0fc,stroke:#333,stroke-width:3px
    style L7 fill:#b197fc,stroke:#333,stroke-width:3px
```

### Layer Responsibilities

| Layer | Component | Status | Description |
|-------|-----------|--------|-------------|
| **L7** | Application | ✅ Complete | `stealth-browser` Helper (SOCKS5 proxy) + Browser Extension UI |
| **L6** | Incentives | 🔮 Future | Payment vouchers, reputation system, resource accounting |
| **L5** | Naming | 🔮 Future | Decentralized identity, alias ledger, self-certifying names |
| **L4** | Privacy | 🔮 Future | Optional mixnet integration (Nym/Sphinx packets) for high anonymity |
| **L3** | Mesh | ✅ Complete | P2P networking via libp2p (mDNS, DHT, circuits, relay) - **Phase 2 done** |
| **L2** | Transport | ✅ Complete | **HTX protocol** - TLS fingerprint cloning + AEAD framing |
| **L1** | Routing | 📋 Deferred | Multi-path selection, path validation (SCION-inspired) - **Post-MVP** |
| **L0** | Physical | ✅ System | OS-provided TCP/UDP/QUIC bearers |

> **Note on L1 Routing**: SCION-inspired path-aware routing is architecturally fundamental for production (path validation, multi-path redundancy, geographic diversity). Currently deferred post-MVP to prioritize user delivery (Phase 3: Browser Extension). Current implementation relies on libp2p's built-in routing (L3) over standard IP (L0), which works but lacks the cryptographic path validation and explicit multi-path control that L1 will provide. **Planned for Phase 4** after extension deployment.

---

## 🚀 How It Works

### High-Level Flow

```mermaid
sequenceDiagram
    participant User as 🧑 User Browser
    participant Ext as 📱 Extension
    participant Helper as 💻 Local Helper
    participant Mesh as 🕸️ QNet Mesh
    participant Exit as 🚪 Exit Node
    participant Target as 🌐 amazon.com
    
    User->>Ext: Browse amazon.com
    Ext->>Helper: SOCKS5 request
    Helper->>Helper: Select decoy (microsoft.com)
    Helper->>Mesh: HTX tunnel to peer<br/>(looks like microsoft.com)
    Mesh->>Exit: Route through P2P mesh
    Exit->>Target: Fetch amazon.com
    Target->>Exit: Response
    Exit->>Mesh: Encrypted response
    Mesh->>Helper: Deliver via tunnel
    Helper->>User: Display content
    
    Note over Helper,Mesh: ISP sees: HTTPS to microsoft.com ✅
    Note over Exit,Target: Reality: Access to amazon.com 🎯
```

### Detailed Connection Flow

#### 1. **Bootstrap & Discovery**
```mermaid
graph TB
    A[Helper Starts] --> B[Load Signed Catalog]
    B --> C{Catalog Valid?}
    C -->|Yes| D[Extract Decoy List + Seeds]
    C -->|No| E[Use Fallback Seeds]
    D --> F[Connect to libp2p DHT]
    E --> F
    F --> G[Discover QNet Peers]
    G --> H[Establish P2P Connections]
    H --> I[Ready to Route Traffic]
    
    style B fill:#ffd43b
    style F fill:#74c0fc
    style I fill:#51cf66
```

**Bootstrap Strategy:**
- **Primary**: Global libp2p DHT (IPFS infrastructure - free, battle-tested)
- **Secondary**: Operator seed nodes (small DigitalOcean droplets, $4-6/month)
- **Updates**: Signed catalog system for adding community nodes
- **Result**: Zero single point of failure

#### 2. **Traffic Masking (HTX Protocol)**

```mermaid
sequenceDiagram
    participant Client as Client
    participant Decoy as Decoy Server<br/>(microsoft.com)
    participant QNet as QNet Peer
    
    Note over Client,Decoy: Phase 1: TLS Mirroring
    Client->>Decoy: ClientHello<br/>(cloned JA3 fingerprint)
    Decoy->>Client: ServerHello + Certificate
    Client->>Decoy: Finished (TLS 1.3)
    
    Note over Client,QNet: Phase 2: Inner HTX Handshake
    Client->>QNet: Noise XK Handshake<br/>(inside TLS stream)
    QNet->>Client: Ephemeral Keys + Static Auth
    
    Note over Client,QNet: Phase 3: Encrypted Data
    Client->>QNet: AEAD Frames<br/>(ChaCha20-Poly1305)
    QNet->>Client: AEAD Frames
    
    Note over Client,Decoy: Observer sees: Normal HTTPS ✅
    Note over Client,QNet: Reality: Encrypted tunnel 🔒
```

**HTX Security Properties:**
- **TLS Fingerprint Cloning**: JA3, ALPN, cipher suites match decoy exactly
- **Inner Noise XK**: Mutual authentication + ephemeral keys
- **AEAD Framing**: ChaCha20-Poly1305 with monotonic nonces
- **Forward Secrecy**: Keys rotate, no persistent state compromise
- **Integrity**: Ed25519 signatures on all config artifacts

#### 3. **Mesh Routing Modes**

```mermaid
graph TB
    subgraph "Fast Mode (1-Hop)"
        U1[User] -->|Direct Tunnel| E1[Exit Node]
        E1 -->|Fetch| T1[Target Site]
    end
    
    subgraph "Privacy Mode (3-Hop)"
        U2[User] -->|HTX| P1[Peer 1]
        P1 -->|Encrypted Relay| P2[Peer 2]
        P2 -->|Encrypted Relay| E2[Exit Node]
        E2 -->|Fetch| T2[Target Site]
    end
    
    style U1 fill:#b197fc
    style U2 fill:#b197fc
    style E1 fill:#51cf66
    style E2 fill:#51cf66
```

**Fast Mode**: Direct tunnel for maximum performance (default)
**Privacy Mode**: Multi-hop relay for stronger anonymity (optional)

#### 4. **Exit Node Architecture**

```mermaid
graph TB
    subgraph "Three-Tier Exit Model"
        direction TB
        T1[Tier 1: User Helpers<br/>Relay-Only Mode<br/>99% of network]
        T2[Tier 2: Operator Exits<br/>DigitalOcean VPS<br/>Primary exits]
        T3[Tier 3: Volunteer Exits<br/>Opt-in community<br/>Advanced users]
    end
    
    T1 -.->|Forward packets<br/>Never decrypt| T2
    T1 -.->|Forward packets<br/>Never decrypt| T3
    T2 -->|Decrypt & fetch| Web[Internet]
    T3 -->|Decrypt & fetch| Web
    
    style T1 fill:#51cf66
    style T2 fill:#ffd43b
    style T3 fill:#74c0fc
```

**Legal Protection Strategy:**
- **Tier 1 (Users)**: Relay-only, no legal risk (can't see content)
- **Tier 2 (Operator)**: Professional VPS with proper abuse policies
- **Tier 3 (Volunteers)**: Explicit opt-in with legal warnings

---

## ✨ Key Features

### 1. Perfect Traffic Disguise

**HTX (Hypertext Transport Extension)** is QNet's secret weapon:

```mermaid
graph LR
    subgraph "What ISP Sees"
        A[Your Computer] -->|HTTPS TLS 1.3| B[microsoft.com]
        B -->|Normal Response| A
    end
    
    subgraph "Reality"
        C[Your Computer] -->|HTX Tunnel| D[QNet Peer]
        D -->|P2P Mesh| E[Exit Node]
        E -->|Real Request| F[Blocked Site]
    end
    
    style B fill:#51cf66
    style D fill:#ffd43b
```

**Technical Implementation:**
- Clones TLS ClientHello fingerprint of decoy site
- Matches JA3, cipher suites, extensions, ALPN
- Traffic timing and padding profiles mimic real usage
- Inner Noise XK handshake provides actual encryption

### 2. Decentralized Peer Discovery

```mermaid
graph TB
    subgraph "Global P2P Mesh"
        H1[Helper Node 1<br/>Asia]
        H2[Helper Node 2<br/>Europe]
        H3[Helper Node 3<br/>Americas]
        H4[Helper Node 4<br/>Africa]
        
        DHT[(libp2p DHT<br/>Kademlia)]
        
        H1 <--> DHT
        H2 <--> DHT
        H3 <--> DHT
        H4 <--> DHT
        
        H1 <-.P2P relay.-> H2
        H2 <-.P2P relay.-> H3
        H3 <-.P2P relay.-> H4
        H4 <-.P2P relay.-> H1
    end
    
    style DHT fill:#ffd43b
```

**No Central Servers:**
- Leverages existing IPFS/libp2p DHT infrastructure
- Fallback to operator seed nodes
- Catalog-based updates for community additions
- Resilient to regional blocking

### 3. Cryptographic Security

**Defense-in-Depth Approach:**

```mermaid
graph TB
    subgraph "Security Layers"
        L1[TLS 1.3 Outer Layer<br/>Decoy Fingerprint]
        L2[Noise XK Handshake<br/>Mutual Authentication]
        L3[AEAD Framing<br/>ChaCha20-Poly1305]
        L4[Ed25519 Signatures<br/>Catalog Integrity]
    end
    
    L1 --> L2
    L2 --> L3
    L3 --> L4
    
    style L1 fill:#ffd43b
    style L2 fill:#74c0fc
    style L3 fill:#51cf66
    style L4 fill:#b197fc
```

**Cryptographic Primitives:**
- **ChaCha20-Poly1305**: AEAD encryption (fast, secure)
- **Ed25519**: Signatures for catalog/config validation
- **X25519**: Ephemeral key exchange (Noise protocol)
- **HKDF-SHA256**: Key derivation

**Security Guarantees:**
- Forward secrecy (ephemeral keys)
- Message integrity (AEAD tags)
- Replay protection (monotonic nonces)
- Tamper detection (signed catalogs)

---

## 🔧 Technology Stack

### Core Technologies

```mermaid
graph TB
    subgraph "Rust Ecosystem"
        Tokio[Tokio<br/>Async Runtime]
        Rustls[Rustls<br/>TLS 1.3]
        Quinn[Quinn<br/>QUIC]
        Ring[ring<br/>Crypto Primitives]
    end
    
    subgraph "Networking"
        Libp2p[libp2p<br/>P2P Framework]
        DHT[Kademlia DHT]
        Gossip[GossipSub]
    end
    
    subgraph "QNet Crates"
        HTX[htx<br/>Traffic Masking]
        Framing[core-framing<br/>AEAD Protocol]
        Crypto[core-crypto<br/>Primitives]
        Mesh[core-mesh<br/>P2P Logic]
    end
    
    Tokio --> HTX
    Rustls --> HTX
    Quinn --> HTX
    Ring --> Crypto
    Libp2p --> Mesh
    DHT --> Mesh
    Gossip --> Mesh
    
    Crypto --> Framing
    Framing --> HTX
    HTX --> Mesh
    
    style Tokio fill:#ffd43b
    style Libp2p fill:#74c0fc
    style HTX fill:#51cf66
```

| Component | Technology | Reason |
|-----------|------------|--------|
| **Core Language** | Rust | Memory safety, performance, fearless concurrency |
| **Async Runtime** | Tokio | Industry-standard async I/O |
| **TLS/QUIC** | Rustls + Quinn | Modern, pure-Rust implementations |
| **P2P Networking** | libp2p | Battle-tested, modular, protocol-agnostic |
| **Cryptography** | ring, ed25519-dalek | Audited, fast, constant-time |
| **Serialization** | CBOR (serde_cbor) | Deterministic encoding for signatures |
| **UI** | WebExtensions API | Cross-browser (Chrome/Edge/Firefox) |

---

## 🛠️ Quick Start (Developers)

### Prerequisites

- **Rust 1.70+**: `rustup install stable`
- **Windows** (primary dev environment) or Linux/macOS
- **PowerShell** (for Windows scripts)

### Build & Run

```powershell
# 1. Clone the repository
git clone https://github.com/QW1CKS/qnet.git
cd qnet

# 2. Build all workspace crates
cargo build --workspace

# 3. Run the Helper (local SOCKS5 proxy)
cargo run -p stealth-browser

# The Helper will start on:
# - SOCKS5 proxy: 127.0.0.1:1088
# - Status API: 127.0.0.1:8088
```

### Verify Installation

```powershell
# Check Helper status
Invoke-WebRequest -Uri http://127.0.0.1:8088/status | ConvertFrom-Json

# Test masked connection (connect to wikipedia disguised as decoy)
pwsh ./scripts/test-masked-connect.ps1 -Target www.wikipedia.org

# Run full test suite
cargo test --workspace

# Run benchmarks (performance-critical crates)
cargo bench -p core-framing
cargo bench -p htx
```

### Development Tools

```powershell
# Format check
cargo fmt --check

# Linting (strict mode)
cargo clippy --workspace --all-targets -- -D warnings

# Fuzz testing (requires nightly)
cargo +nightly fuzz run framing_fuzz

# Spec validation (Go linter)
cd linter
go build -o qnet-lint ./cmd/qnet-lint
./qnet-lint validate ..
```

---

## 📁 Project Structure

```
qnet/
├── apps/                      # User-facing applications
│   ├── stealth-browser/       # 💻 Helper Node (SOCKS5 + status API)
│   └── edge-gateway/          # 🚪 Server-side exit node
│
├── crates/                    # Core Rust libraries
│   ├── htx/                   # 🎭 HTX protocol (TLS mirroring)
│   ├── core-framing/          # 📦 AEAD frame encoding/decoding
│   ├── core-crypto/           # 🔐 Cryptographic primitives
│   ├── core-mesh/             # 🕸️ P2P mesh networking (libp2p)
│   ├── core-routing/          # 🗺️ Path selection (future)
│   ├── core-mix/              # 🎲 Mixnet integration (future)
│   ├── alias-ledger/          # 📛 Decentralized naming (future)
│   ├── voucher/               # 💰 Payment system (future)
│   └── catalog-signer/        # ✍️ Catalog signing tool
│
├── qnet-spec/                 # Specification & governance
│   ├── specs/001-qnet/
│   │   ├── spec.md            # 📖 Protocol specification
│   │   ├── plan.md            # 🗺️ Strategic roadmap
│   │   └── tasks.md           # ✅ Unified task list
│   ├── memory/
│   │   ├── ai-guardrail.md    # 🤖 AI coding guidelines
│   │   └── testing-rules.md   # 🧪 Testing requirements
│   └── docs/                  # Component documentation
│
├── docs/                      # Architecture documentation
│   ├── ARCHITECTURE.md        # 🏗️ System architecture
│   ├── CONTRIBUTING.md        # 🤝 Contribution guide
│   └── helper.md              # 📚 Helper API reference
│
├── tests/                     # Integration tests
├── fuzz/                      # Fuzzing targets
├── scripts/                   # Automation scripts
└── artifacts/                 # Benchmarks & performance data
```

### Key Crates

| Crate | Purpose | Status |
|-------|---------|--------|
| `htx` | HTX protocol implementation (TLS mirroring + Noise) | ✅ Complete |
| `core-framing` | AEAD frame encoding (ChaCha20-Poly1305) | ✅ Complete |
| `core-crypto` | Cryptographic wrappers (Ed25519, X25519, HKDF) | ✅ Complete |
| `core-cbor` | Deterministic CBOR serialization | ✅ Complete |
| `core-mesh` | P2P networking via libp2p (mDNS, DHT, circuits) | ✅ Complete |
| `core-routing` | L1 multi-path routing (SCION-inspired) | 📋 Deferred |
| `core-mix` | Mixnet integration (Sphinx packets) | 🔮 Future |
| `alias-ledger` | Self-certifying identities | 🔮 Future |
| `voucher` | Micropayment vouchers | 🔮 Future |

---

## 🔒 Security Model

### Threat Model

QNet is designed to resist:

```mermaid
graph TB
    subgraph "Adversary Capabilities"
        A1[Passive Network Observer<br/>DPI, Traffic Analysis]
        A2[Active MITM<br/>TLS Interception]
        A3[Censorship Middlebox<br/>Protocol Filtering]
        A4[Regional Blocking<br/>IP/Domain Blacklists]
    end
    
    subgraph "QNet Defenses"
        D1[TLS Fingerprint Cloning<br/>Indistinguishable Traffic]
        D2[Inner Noise XK Protocol<br/>Mutual Authentication]
        D3[Decoy Host Diversity<br/>Trusted Domains]
        D4[P2P Mesh Routing<br/>No Fixed Infrastructure]
    end
    
    A1 -.blocked by.-> D1
    A2 -.blocked by.-> D2
    A3 -.blocked by.-> D3
    A4 -.blocked by.-> D4
    
    style A1 fill:#ff6b6b
    style A2 fill:#ff6b6b
    style A3 fill:#ff6b6b
    style A4 fill:#ff6b6b
    style D1 fill:#51cf66
    style D2 fill:#51cf66
    style D3 fill:#51cf66
    style D4 fill:#51cf66
```

### Security Properties

| Property | Implementation | Verification |
|----------|----------------|--------------|
| **Confidentiality** | ChaCha20-Poly1305 AEAD | Constant-time crypto libs |
| **Integrity** | AEAD tags + Ed25519 signatures | Tamper-detection tests |
| **Forward Secrecy** | Ephemeral X25519 keys (Noise XK) | Key rotation tests |
| **Replay Protection** | Monotonic nonces | Nonce uniqueness tests |
| **Traffic Masking** | TLS fingerprint cloning | DPI capture validation |
| **Catalog Integrity** | Ed25519 + DET-CBOR | Signature verification tests |

### Security Best Practices

```mermaid
graph TB
    subgraph "Development Security"
        S1[No Hardcoded Keys]
        S2[Constant-Time Crypto]
        S3[Secure RNG]
        S4[Memory Wiping]
    end
    
    subgraph "Operational Security"
        S5[Signed Catalogs]
        S6[Pinned Public Keys]
        S7[Version Monotonicity]
        S8[Audit Logging]
    end
    
    S1 --> S5
    S2 --> S6
    S3 --> S7
    S4 --> S8
    
    style S1 fill:#ffd43b
    style S2 fill:#ffd43b
    style S5 fill:#74c0fc
    style S6 fill:#74c0fc
```

**Key Invariants:**
- All cryptographic operations use vetted libraries (`ring`, `ed25519-dalek`)
- No secret-dependent branching (constant-time guarantees)
- Nonce uniqueness enforced via monotonic counters
- Signed config objects validated before use
- Expired catalogs rejected with grace period

---

## ⚡ Performance

### Benchmarks

**Environment**: Intel Core i7, 16GB RAM, Windows 11

| Operation | Throughput | Latency |
|-----------|------------|---------|
| **HTX Handshake** | - | ~50ms (incl. TLS) |
| **AEAD Frame Encoding** | 2.5 GB/s | ~400 ns/frame |
| **AEAD Frame Decoding** | 2.3 GB/s | ~430 ns/frame |
| **Catalog Verification** | - | ~2ms (Ed25519) |
| **1-Hop Connection** | 80-120 Mbps | +5-15ms vs direct |
| **3-Hop Connection** | 40-80 Mbps | +20-50ms vs direct |

**Performance Optimization:**
- Zero-copy frame processing where possible
- Reusable buffer pools (no per-frame allocation)
- Vectorized crypto operations (SIMD when available)
- Connection multiplexing (reduce handshake overhead)

### Scalability

```mermaid
graph LR
    subgraph "Network Growth"
        N1[100 Nodes] -->|More Peers| N2[1,000 Nodes]
        N2 -->|More Routes| N3[10,000 Nodes]
        N3 -->|More Capacity| N4[100,000+ Nodes]
    end
    
    subgraph "Benefits"
        B1[More Exit Diversity]
        B2[Better Geographic Coverage]
        B3[Higher Aggregate Bandwidth]
        B4[Stronger Censorship Resistance]
    end
    
    N4 --> B1
    N4 --> B2
    N4 --> B3
    N4 --> B4
    
    style N4 fill:#51cf66
```

**Scalability Design:**
- DHT-based discovery (logarithmic routing)
- Gossip protocol for mesh updates (epidemic spread)
- Local routing tables (no global state)
- Lazy connection management (connect on-demand)

---

## 📚 Documentation

> [!WARNING]
> This documentation is a work in progress. Please refer to the [qnet-spec/](qnet-spec/) directory for the most up-to-date technical specifications and design documents.

### For Users
- **[Quick Start Guide](docs/QUICKSTART.md)** - Get running in 5 minutes
- **[Browser Extension Guide](qnet-spec/docs/extension.md)** - Using the UI
- **[Troubleshooting](docs/TROUBLESHOOTING.md)** - Common issues

### For Developers
- **[Architecture Overview](docs/ARCHITECTURE.md)** - System design
- **[Protocol Specification](qnet-spec/specs/001-qnet/spec.md)** - Wire format details
- **[Contributing Guide](docs/CONTRIBUTING.md)** - How to contribute
- **[Testing Rules](qnet-spec/memory/testing-rules.md)** - Test requirements
- **[AI Guardrails](qnet-spec/memory/ai-guardrail.md)** - AI coding standards

### For Operators
- **[Running an Exit Node](docs/EXIT_NODE.md)** - Deployment guide
- **[Catalog Management](docs/CATALOG.md)** - Signing & distribution
- **[Security Best Practices](SECURITY.md)** - Hardening guide

### Specification Documents
- **[Unified Task List](qnet-spec/specs/001-qnet/tasks.md)** - Development roadmap
- **[Strategic Plan](qnet-spec/specs/001-qnet/plan.md)** - Vision & phases
- **[Constitution](qnet-spec/specs/001-qnet/constitution.md)** - Governance principles

---

## 🤝 Contributing

We welcome contributions! QNet is building the future of internet freedom.

### How to Contribute

```mermaid
graph LR
    A[Pick a Task] --> B[Create Branch]
    B --> C[Write Tests]
    C --> D[Implement]
    D --> E[Run Checks]
    E --> F[Submit PR]
    
    style A fill:#ffd43b
    style C fill:#74c0fc
    style F fill:#51cf66
```

**Step-by-Step:**

1. **Find a Task**: Check [tasks.md](qnet-spec/specs/001-qnet/tasks.md) for open items
   - Look for Phase 2 (Helper development) or Phase 3 (User experience)
   - Comment on the task to claim it

2. **Set Up Environment**:
   ```powershell
   git clone https://github.com/QW1CKS/qnet.git
   cd qnet
   cargo build --workspace
   cargo test --workspace
   ```

3. **Development Workflow**:
   - Add/update tests first (test-driven development)
   - Implement minimal changes (trace to spec task)
   - Run checks: `cargo fmt`, `cargo clippy`, `cargo test`
   - Verify fuzz targets if touching parsers

4. **Commit Requirements**:
   ```
   Brief description of change

   - Detailed point 1
   - Detailed point 2

   Task: T3.2 (example)
   AI-Guardrail: PASS
   Testing-Rules: PASS
   ```

5. **Pull Request**:
   - Include spec/task references
   - Attach before/after benchmarks (if performance-sensitive)
   - Explain risk assessment
   - No unrelated refactors

### Contribution Areas

| Area | Skills | Difficulty |
|------|--------|------------|
| **HTX Protocol** | Rust, TLS, Cryptography | 🔴 Hard |
| **Mesh Networking** | Rust, libp2p, P2P | 🟡 Medium |
| **Helper/Extension** | Rust, JavaScript, UI | 🟢 Easy |
| **Testing** | Any language, QA mindset | 🟢 Easy |
| **Documentation** | Technical writing | 🟢 Easy |
| **Performance** | Profiling, optimization | 🟡 Medium |

### Code Standards

- **Language**: Idiomatic Rust (follow existing patterns)
- **Formatting**: `cargo fmt --check` (enforce)
- **Linting**: `cargo clippy` with `-D warnings`
- **Testing**: ≥80% coverage for critical paths
- **Security**: Follow [AI guardrails](qnet-spec/memory/ai-guardrail.md)

---

## 🗺️ Roadmap

```mermaid
gantt
    title QNet Development Timeline
    dateFormat YYYY-MM-DD
    section Phase 1 ✅
    Core Infrastructure    :done, p1, 2025-09-15, 2025-10-31
    HTX Protocol          :done, p1a, 2025-09-15, 2025-10-15
    Crypto & Framing      :done, p1b, 2025-09-20, 2025-10-20
    Catalog System        :done, p1c, 2025-10-01, 2025-10-25
    
    section Phase 2 ✅
    Peer Discovery (2.1)  :done, p2a, 2025-10-15, 2025-11-01
    Relay Logic (2.2)     :done, p2b, 2025-11-01, 2025-11-10
    Circuit Building (2.3):done, p2c, 2025-11-10, 2025-11-20
    Helper Integration (2.4):done, p2d, 2025-11-20, 2025-11-26
    Exit Infrastructure (2.5):p2e, 2025-11-27, 2025-12-15
    
    section Phase 3 🚧
    Browser Extension     :p3, 2025-12-01, 2026-02-28
    Native Messaging      :p3a, 2025-12-15, 2026-01-15
    UI/UX Development     :p3b, 2026-01-01, 2026-02-15
    Installers & Packaging:p3c, 2026-02-01, 2026-03-01
    
    section Phase 4 🔮
    L1 Path Routing       :p4a, 2026-03-01, 2026-05-31
    Mixnet Integration    :p4b, 2026-04-01, 2026-07-31
    Payment System        :p4c, 2026-06-01, 2026-09-30
    Governance            :p4d, 2026-07-01, 2026-10-31
```

### Current Status: Phase 2 Core Complete (80%) → Phase 2.5 & 3 Next

**Phase 1: Core Infrastructure** (✅ 100% Complete - Sept 15 - Oct 31, 2025)
- ✅ HTX protocol implementation (`htx/`)
- ✅ AEAD framing layer (`core-framing/`)
- ✅ Cryptographic primitives (`core-crypto/`)
- ✅ Catalog signing system (`catalog-signer/`)
- ✅ Deterministic CBOR encoding (`core-cbor/`)

**Phase 2: P2P Mesh Network** (✅ 80% Complete - Oct 15 - Nov 26, 2025)

*Completed Sections (2.1-2.4):*
- ✅ **2.1 Peer Discovery** - mDNS local + Kademlia DHT + public IPFS bootstrap
- ✅ **2.2 Relay Logic** - Packet forwarding, routing table, statistics tracking  
- ✅ **2.3 Circuit Building** - Multi-hop circuits (max 3 hops), auto-teardown
- ✅ **2.4 Helper Integration** - SOCKS5→Mesh tunneling, status API, CLI modes

*Pending (2.5-2.6):*
- 🚧 **2.5 Infrastructure** - Exit node deployment scripts, bandwidth policies, operator droplets
- 📋 **2.6 Production Checkpoint** - Security audit, 24hr stability test, performance validation

**Phase 3: User Experience** (📋 0% - Starting Dec 2025)
- 📋 Browser extension UI (React/Preact)
- 📋 Native messaging bridge (Helper ↔ Extension)
- 📋 Cross-platform installers (Windows/Linux/macOS)
- 📋 User documentation & onboarding guides

**Phase 4: Advanced Features** (🔮 Future - Q2 2026+)
- 📋 L1 SCION-inspired path routing (cryptographic path validation)
- 📋 Mixnet privacy hops (Nym/Sphinx integration)
- 📋 Micropayment system (vouchers, relay incentives)
- 📋 Decentralized governance (voting, upgrades)
- 📋 Mobile support (Android/iOS apps)

---

## 📜 License

QNet is released under the **MIT License**.

```
MIT License

Copyright (c) 2024 QNet Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software...
```

See [LICENSE](LICENSE) for full text.

---

## 🙏 Acknowledgments

QNet builds on the shoulders of giants:
- **Tor Project**: Pioneering anonymous communication
- **IPFS/libp2p**: Decentralized networking protocols
- **Rustls**: Modern TLS implementation
- **Nym**: Mixnet research and implementation
- **SCION**: Secure path-aware networking

---

## 📞 Contact & Community

- **GitHub Issues**: Bug reports and feature requests
- **Discussions**: Design discussions and Q&A
- **Security**: See [SECURITY.md](SECURITY.md) for responsible disclosure

---

<div align="center">
  <p><strong>Building the unblockable internet, one node at a time.</strong></p>
  <p>⭐ Star us on GitHub | 🍴 Fork and contribute | 📢 Spread the word</p>
</div>