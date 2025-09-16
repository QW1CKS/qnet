# QNet: Decentralized, Censorship-Resistant Networking

<div align="center">
  <img src="logo.png" alt="QNet Logo" width="400">
</div>

<p align="center">
  <strong>A decentralized network protocol stack designed to replace the vulnerable public Internet with a privacy-preserving, self-sovereign alternative.</strong>
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#documentation">Documentation</a> •
  <a href="#contributing">Contributing</a> •
  <a href="#license">License</a>
</p>

---

## 🌟 Overview

**QNet (QuantaNet)** is a decentralized network protocol designed to replace the vulnerable public Internet. It provides **strong user privacy**, **decentralized operation**, and **resistance to censorship** through a sophisticated layered architecture and advanced cryptography.

### ✨ Key Features

- **🔒 Censorship-Proof**: Route around blocks with stealthy, mixnet-powered tunnels
- **🛡️ Privacy-First**: Traffic mimics normal HTTPS, undetectable by ISPs or governments
- **🌐 Decentralized**: No central authorities, no DNS vulnerabilities, no single points of failure
- **⚡ Scalable & Secure**: Built for millions of users with quantum-resistant cryptography
- **🔧 Developer-Friendly**: Modular crates for building custom privacy tools
- **📦 Catalog-First**: Signed configuration catalogs with automatic updates (M3 complete)

### 🎯 Real-World Impact

- **Journalists & Activists**: Access blocked sites without detection
- **Developers**: Build censorship-resistant apps effortlessly
- **Everyday Users**: Browse freely, no matter where you are
- **Governments & ISPs**: Can't block what they can't see!

---

## Architecture

QNet's innovative **7-layer architecture** ensures seamless, secure connectivity:

| Layer | Name | Purpose | Implementation |
|-------|------|---------|----------------|
| **L7** | Applications | Web-layer replacement services | Ready-to-use apps |
| **L6** | Payments | Micro-payments via Lightning | Voucher system |
| **L5** | Naming & Trust | Self-certifying IDs + alias ledger | Decentralized naming |
| **L4** | Privacy Hops | Nym mixnet for anonymity | Mixnode integration |
| **L3** | Overlay Mesh | P2P connections via libp2p | Peer discovery |
| **L2** | Cover Transport | HTX over TCP-443/QUIC-443 | TLS mirroring |
| **L1** | Path Selection | SCION + HTX routing | Secure routing |
| **L0** | Access Media | Any IP bearer | OS integration |

### 🛠️ Technology Stack

**Core Technologies:**
- **Rust**: Memory-safe, high-performance networking
- **WebExtensions + Helper**: Browser Extension (Chrome/Edge/Firefox) + local Helper service for user delivery
- **Tokio**: Async runtime for concurrency
- **Ring**: Cryptographic primitives
- **Libp2p**: P2P networking
- **Nym**: Privacy mixnet integration

**Cryptography:**
- ChaCha20-Poly1305 AEAD encryption
- Ed25519 signatures, X25519 DH, HKDF-SHA256
- Post-quantum hybrid X25519-Kyber768 (2027+)
- Noise XK mutual authentication

---

## Quick Start

### Prerequisites

- **Rust 1.70+** with Cargo
- **Windows**: Visual Studio Build Tools 2022 (C++ workload + Windows SDK)
- **Linux/macOS**: Standard development tools

### Demo: Browser Extension + Helper (recommended for users)

For end users we recommend a lightweight browser extension paired with a small local Helper application (the "Helper"). This model provides the best user experience and security while leveraging the existing Rust networking components.

What you get:
- A browser extension (Chrome/Edge/Firefox) that provides a one-click UI and proxy control
- A small Helper application (Rust binaries) installed once on the host that runs the `stealth-browser` SOCKS proxy and optional `edge-gateway` service

Quick start (developer/demo): run the Helper locally and use the browser configured to use the local proxy.

**Windows (PowerShell) — run stealth-browser directly for development:**
```powershell
# From repo root (development only)
cargo run -p stealth-browser
```

**Smoke Test** (from another terminal — verifies local proxy):
```bash
curl -I https://example.com --socks5-hostname 127.0.0.1:1088
```

Notes:
- The extension automates launching the Helper and configuring the browser to use `127.0.0.1:1088` (default) for SOCKS5.
- For production distribution, the Helper should be packaged as an installer (Windows MSI, macOS PKG) and shipped alongside the browser extension.

### Demo: Secure Connection

Demonstrate a full secure connection with catalog-first configuration (signed + bundled), TLS handshake, decoy routing, and DPI evasion verification:

**Windows (PowerShell):**
```powershell
# Run one-click demo with DPI capture (catalog-first by default)
.\scripts\demo-secure-connection.ps1 `
  -WithDecoy `
  -Origin https://www.wikipedia.org `
  -CaptureSeconds 60 `
   -Interface 3
```

**Features:**
- ✅ Catalog-first configuration (signed, bundled; updates from mirrors)
- ✅ Real TLS handshake + inner HTX secure stream
- ✅ Decoy routing for censorship evasion
- ✅ DPI capture and comparison (PASS if traffic looks like normal TLS)
- ✅ Edge gateway for production masked browsing (M3 complete)

See [Demo: Secure Connection](docs/DEMO_SECURE_CONNECTION.md) for full details and troubleshooting, and [Catalog Schema](qnet-spec/docs/catalog-schema.md) for the signed catalog format. For user components, see the [Helper Guide](qnet-spec/docs/helper.md) and [Browser Extension Guide](qnet-spec/docs/extension.md).

### Development Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/QW1CKS/qnet.git
   cd qnet
   ```

2. **Build the workspace:**
   ```bash
   cargo build --workspace
   ```

3. **Run tests:**
   ```bash
   cargo test --workspace
   ```

4. **Run examples:**
   ```bash
   cargo run -p echo
   ```

---

## Documentation

### 📖 Specifications
- **[QNet Specification](qnet-spec/specs/001-qnet/spec.md)**: Complete technical specification
- **[Implementation Plan](qnet-spec/specs/001-qnet/plan.md)**: Development roadmap and phases
- **[Task Tracker](qnet-spec/specs/001-qnet/tasks.md)**: Detailed implementation tasks

### 🏛️ Project Governance
- **[Constitution](qnet-spec/memory/constitution.md)**: Core principles and governance
- **[AI Guardrail](qnet-spec/memory/ai-guardrail.md)**: Code quality standards
- **[Testing Rules](qnet-spec/memory/testing-rules.md)**: Testing requirements

### 🏗️ Technical Documentation
- **[Architecture](qnet-spec/docs/ARCHITECTURE.md)**: System architecture details
- **[Contributing](qnet-spec/docs/CONTRIBUTING.md)**: Development guidelines
- **[Demo: Secure Connection](docs/DEMO_SECURE_CONNECTION.md)**: Step-by-step secure connection demo
- **[Helper Guide](qnet-spec/docs/helper.md)**: Local Helper (stealth-browser) install, endpoints, and API
- **[Browser Extension Guide](qnet-spec/docs/extension.md)**: Extension developer flow and integration
- **[Physical Testing Playbook](qnet-spec/docs/physical-testing.md)**: Hands-on validation with captures and metrics
- **[API Documentation](https://docs.rs/qnet)**: Generated Rust docs

### 🧪 Development Tools
- **Go Spec Linter**: Compliance validation tool
- **uTLS Generator**: TLS template generator
- **Performance Benchmarks**: Criterion-based testing

---

## 👥 For Developers

QNet is primarily a **toolkit and framework** for developers to build private, censorship-resistant networks and applications.

### Core Crates

| Crate | Purpose | Status |
|-------|---------|--------|
| `core-crypto` | Cryptographic primitives | ✅ Complete |
| `core-cbor` | Deterministic CBOR encoding | ✅ Complete |
| `core-framing` | L2 frame handling | ✅ Complete |
| `htx` | HTTP Tunneling Extension | ✅ Complete (M3 catalog pipeline) |
| `core-routing` | SCION routing | 🚧 In Progress |
| `core-mesh` | Libp2p integration | 🚧 In Progress |

### Integration Example

```rust
use htx::api::{dial};

// Dial with TLS origin mirroring
let conn = dial("https://example.com")?;

// Open secure stream
let stream = conn.open_stream();
stream.write(b"hello world");
```

### Development Workflow

1. **Map changes to requirements** in `qnet-spec/specs/001-qnet/tasks.md`
2. **Follow TDD**: Write tests first, then implement
3. **Ensure compliance** with AI Guardrail and Testing Rules
4. **Submit PRs** with `AI-Guardrail: PASS` and `Testing-Rules: PASS`

---

## 👤 For Users

QNet provides a user-friendly deployment model composed of a browser extension and a small local Helper application. This approach minimizes installation friction while providing robust masking and catalog updates.

### How users install and use QNet (recommended)

1. Install the QNet browser extension from the browser's extension store (Chrome Web Store, Firefox Add-ons, or Edge Add-ons).
2. Download and install the QNet Helper for your platform (small installer that contains the Rust binaries).
3. Open the browser and click the QNet extension icon. The extension will:
   - Detect and start the local Helper if needed (or prompt for the Helper installer)
   - Configure the browser's proxy settings to use the local SOCKS5 proxy (default 127.0.0.1:1088)
   - Show status (connected, catalog version, toggle protection)
4. Browse normally — QNet masks your connection using decoys from the catalog so observers see the decoy domain instead of your real destination.

More details: [Helper Guide](qnet-spec/docs/helper.md) • [Browser Extension Guide](qnet-spec/docs/extension.md)

### Notes for power users / developers

- Developers can still run `stealth-browser` directly from source for debugging:

```powershell
# Development only: run local proxy
cargo run -p stealth-browser
```

- For production the Helper should be packaged and installed once; the extension handles starting/stopping it and configuring the browser.

---

## 🔧 Advanced Usage

### TLS Origin Mirroring Demo

Test TLS fingerprint mirroring:

```bash
# With rustls ClientConfig
cargo run -p htx --features rustls-config --example tls_mirror_demo -- https://example.com

# Without ClientConfig (fingerprint only)
cargo run -p htx --example tls_mirror_demo -- https://example.com
```

### Performance Testing

Run comprehensive benchmarks:

```bash
# Full performance suite
cargo bench

# Specific benchmarks
cargo bench --bench core-crypto
cargo bench --bench htx
```

### Compliance Testing

Validate implementation against specification:

```bash
# Build Go linter
cd linter && go build -o qnet-lint ./cmd/qnet-lint

# Validate codebase
./qnet-lint validate /path/to/qnet

# Generate SBOM
./qnet-lint sbom /path/to/qnet
```

---

## Contributing

We welcome contributions from developers, security researchers, and protocol designers!

### Getting Started

1. **Review Requirements:**
   - Read `qnet-spec/memory/constitution.md`
   - Study `qnet-spec/memory/ai-guardrail.md`
   - Follow `qnet-spec/memory/testing-rules.md`

2. **Development Setup:**
   ```bash
   git clone https://github.com/QW1CKS/qnet.git
   cd qnet
   cargo build --workspace
   cargo test --workspace
   ```

3. **Find Tasks:**
   - Check `qnet-spec/specs/001-qnet/tasks.md`
   - Look for `TODO` comments in code
   - Review open issues

### Contribution Guidelines

- **Map changes** to `qnet-spec/specs/001-qnet` requirements
- **Write tests first** (TDD approach)
- **Follow AI Guardrail** and Testing Rules
- **Include checklists** in PR descriptions
- **Keep code idiomatic** and well-documented

---

## 📊 Project Status

This project serves two audiences. We now track progress along two parallel tracks: Toolkit (protocol crates, performance, compliance) and User Delivery (Browser Extension + Helper packaging and UX).

### Toolkit track — Implementation Progress

- ✅ **Phase 1**: Core Infrastructure (Complete)
- ✅ **Phase 2**: HTX Proof-of-Concept (90% Complete - M3 catalog pipeline done)
- 🚧 **Phase 3**: Routing & Mesh (In Progress)
- ⏳ **Phase 4**: Privacy & Naming (Planned)
- ⏳ **Phase 5**: Payments & Governance (Planned)

### User delivery track — Extension + Helper

- **U1**: Helper service (stealth-browser SOCKS5 127.0.0.1:1088; status API 127.0.0.1:8088) ✅
- **U2**: Browser Extension MVP (UI + proxy toggle + native messaging handshake) 🚧
- **U3**: Catalog-first integration surfaced in UI (signed updates, status) ✅
- **U4**: Store submissions and Helper installers (Win/macOS/Linux) ⏳

### Performance Benchmarks

| Component | Metric | Target | Current |
|-----------|--------|--------|---------|
| AEAD Throughput | 16KiB blocks | ≥1.2 GiB/s | ✅ 1.2-1.35 GiB/s |
| HTX Handshake | Latency | <50ms | ✅ ~750µs |
| Frame Processing | 16KiB | <12µs | ✅ ~11-12µs |

---

## 🔒 Security

QNet implements multiple layers of security:

- **AEAD Encryption**: ChaCha20-Poly1305 for all data
- **Mutual Authentication**: Noise XK handshake
- **Path Validation**: SCION signature verification
- **Post-Quantum Ready**: Hybrid cryptography (2027+)
- **Anti-Correlation**: Cover traffic and fallback mechanisms
- **Deterministic Serialization**: Prevents parsing attacks

### Security Audits

- 🔍 **Code Review**: Required for all cryptographic components
- 🧪 **Fuzz Testing**: 80%+ coverage target for parsers
- 📋 **Compliance**: Automated spec validation
- 🔐 **SLSA Provenance**: Reproducible builds

---

## License

QNet is licensed under the **MIT License**. See [LICENSE](LICENSE) file for details.

---

## 🌍 Community

- **📖 [Documentation](qnet-spec/docs/)**: Comprehensive guides and API references
- **🐛 [Issues](https://github.com/QW1CKS/qnet/issues)**: Bug reports and feature requests
- **💬 [Discussions](https://github.com/QW1CKS/qnet/discussions)**: General discussion and Q&A
- **📧 [Security](SECURITY.md)**: Security vulnerability reporting

---