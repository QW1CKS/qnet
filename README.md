<div align="center">
  <img src="logo.png" alt="QNet Logo" width="400">
</div>

# QNet

QNet is a decentralized, censorship-resistant network protocol stack designed to replace traditional web infrastructure with a privacy-preserving, self-sovereign alternative. It enables secure, anonymous communication over any IP bearer, from fiber to satellite, without relying on centralized authorities or vulnerable DNS systems.

## Overview

QNet provides a layered architecture for building censorship-resistant applications:

- **L0 Access Media**: Agnostic to underlying transport (IP, LoRa, satellite, etc.)
- **L1 Path Selection & Routing**: Secure routing with SCION and HTX-tunnelled transitions
- **L2 Cover Transport**: HTX over TLS/QUIC on standard ports (443) with origin mirroring
- **L3 Overlay Mesh**: Peer-to-peer connections via libp2p
- **L4 Privacy Hops**: Optional anonymity through Nym mixnet
- **L5 Naming & Trust**: Self-certifying identities and alias ledger
- **L6 Payments**: Voucher-based micro-payments with Lightning integration
- **L7 Applications**: Web-layer replacement services

Key features include:
- Origin-mirrored TLS handshakes for indistinguishability
- Noise XK inner channel with post-quantum hybrid cryptography
- Deterministic CBOR encoding for reproducible serialization
- SCION-based path validation and routing
- Mixnode selection with diversity constraints
- Anti-correlation measures and cover traffic
- Reproducible builds with SLSA provenance

## Goals

- **Censorship Resistance**: Operate without single points of failure
- **User Privacy**: Strong anonymity and metadata minimization
- **Decentralization**: No central authorities for naming or routing
- **Interoperability**: Compatible with existing internet infrastructure
- **Scalability**: Support millions of users and services
- **Security**: Quantum-resistant cryptography and formal verification

## Architecture

QNet is implemented as a modular Rust workspace with shared crates:

```
qnet/
├── core-crypto/        # Cryptographic primitives (ChaCha20, Ed25519, etc.)
├── core-cbor/          # Deterministic CBOR encoding
├── core-framing/       # L2 frame handling
├── htx/                # HTX client/server implementation
├── mixnode/            # Nym mixnet integration
├── c-lib/              # C library wrapper
├── linter/             # Go-based compliance checker
├── utls-gen/           # uTLS template generator
├── docs/               # Documentation and guides
└── examples/           # Sample applications
```

## Technology Stack

- **Primary Language**: Rust (for safety, performance, async)
- **Cryptography**: ring (ChaCha20-Poly1305, Ed25519, X25519, HKDF)
- **Async Runtime**: tokio
- **TLS/QUIC**: rustls, quinn
- **P2P**: libp2p
- **Routing**: SCION (Rust implementation)
- **Mixnet**: Nym SDK
- **Serialization**: serde with CBOR
- **Database**: sled (embedded)
- **CI/CD**: GitHub Actions with SLSA provenance
- **Containerization**: Docker

## Development Phases

### Phase 1: Core Infrastructure (High Priority)
- Project scaffolding and Rust workspace setup
- Cryptographic primitives and deterministic CBOR
- L2 frame handling and Noise XK handshake
- HTX crate API with dial/accept

### Phase 2: HTX Proof-of-Concept (High Priority)
- TLS origin mirroring and calibration
- Inner channel establishment
- Frame multiplexing and flow control
- Fuzzing, testing, and compliance harness

### Phase 3: Routing & Mesh (Medium Priority)
- SCION packet structures
- HTX tunneling for non-SCION links
- libp2p integration and peer discovery
- Bootstrap mechanisms with adaptive PoW

### Phase 4: Privacy & Naming (Medium Priority)
- Mixnode selection and Nym integration
- Self-certifying identities
- Alias ledger with multi-chain finality

### Phase 5: Payments & Governance (Low Priority)
- Voucher system with FROST signatures
- Governance scoring and voting

### Phase 6: Tools & Compliance (Medium Priority)
- C library implementation
- Go spec linter
- uTLS template generator
- SLSA provenance pipeline

## Spec Kit

This repository uses a structured specification kit (`qnet-spec/`) for managing requirements, plans, and tasks:

- `specs/001-qnet/spec.md`: High-level requirements and user stories
- `specs/001-qnet/plan.md`: Technical implementation plan
- `specs/001-qnet/tasks.md`: Detailed task breakdown with priorities
- `memory/constitution.md`: Project principles and governance
- `templates/`: Reusable templates for new features
- `scripts/`: Helper scripts for development workflow

The spec kit ensures all changes are driven by documented requirements and maintainable through modular, testable components.

## Getting Started

### Prerequisites
- Rust 1.70+ with Cargo
- Go 1.21+ (for linter)
- Docker (for builds and testing)
- Linux/Windows development environment

### Windows prerequisites
- Install Visual Studio Build Tools 2022 with C++ and the Windows SDK:
  - Required components: "MSVC v143 - VS 2022 C++ x64/x86 build tools" and a "Windows 10/11 SDK".
  - Install via GUI (Build Tools installer) or with Winget:

```powershell
# Install Visual Studio Build Tools (opens installer UI to select components)
winget install --id Microsoft.VisualStudio.2022.BuildTools -e

# Ensure Rust MSVC toolchain is installed and selected
rustup toolchain install stable-x86_64-pc-windows-msvc
rustup default stable-x86_64-pc-windows-msvc

# Open "Developer PowerShell for VS 2022" (recommended) and build
cargo build --workspace --all-targets
cargo test --workspace --all-features
```

If you see "LNK1181: cannot open input file 'kernel32.lib'": launch the Build Tools installer and add a Windows SDK, then retry from a Developer PowerShell.

### Building
```bash
# Clone the repository
git clone https://github.com/QW1CKS/qnet.git
cd qnet

# Build the Rust workspace
cargo build --release

# Run tests
cargo test

# Build Docker image
docker build -t qnet .
```

## Quick local run

Below are minimal commands to run a working example locally. Use the PowerShell block on Windows or the Bash block on Linux/macOS.

Windows (PowerShell):
```powershell
# From the repo root
cargo test --workspace

# TLS origin mirroring demo (with rustls client config)
cargo run -p htx --features rustls-config --example tls_mirror_demo -- https://www.cloudflare.com

# Echo placeholder example
cargo run -p echo
```

Linux/macOS (Bash):
```bash
# From the repo root
cargo test --workspace

# TLS origin mirroring demo (with rustls client config)
cargo run -p htx --features rustls-config --example tls_mirror_demo -- https://www.cloudflare.com

# Echo placeholder example
cargo run -p echo
```

What you’ll see (example):
- origin: the URL you probed
- template_id: 32-byte ID (hex) of the mirrored TLS template
- alpn: negotiated protocol preferences (e.g., ["h2", "http/1.1"]) 
- ja3: JA3 fingerprint hash derived from the template
- rustls_cfg_alpn: ALPN list embedded into the rustls ClientConfig (when feature enabled)

Notes:
- The rustls-config feature embeds a ready-to-use rustls::ClientConfig in the example output. If you don’t need it, omit --features rustls-config.
- On first run for a host, a 24h in-memory cache of the template is populated.

### Using the HTX API (experimental)

Enable the rustls-config feature to use the TLS-backed dial() API. This performs origin calibration, builds a rustls ClientConfig, derives inner keys via TLS exporter bound to the TemplateID and Caps, then opens a multiplexed secure connection.

Windows (PowerShell):
```powershell
# Build and run tests with TLS integration
cargo test -p htx --features rustls-config

# Example: in-process secure connection demo
cargo test -p htx api::tests::api_echo_e2e
```

Usage sketch:
```rust
use htx::api::{dial};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Requires --features rustls-config
  let conn = dial("https://www.cloudflare.com")?;
  let s = conn.open_stream();
  s.write(b"hello");
  if let Some(resp) = s.read() {
    println!("got {} bytes", resp.len());
  }
  Ok(())
}
```
Feature flag:
- The dial() function is only available with `--features rustls-config`. Without it, you’ll get ApiError::FeatureDisabled.

### Running Examples
- TLS origin mirroring demo:
  - With rustls ClientConfig embedded:
    - Windows (PowerShell):
      ```powershell
      cargo run -p htx --features rustls-config --example tls_mirror_demo -- https://example.com
      ```
    - Linux/macOS (Bash):
      ```bash
      cargo run -p htx --features rustls-config --example tls_mirror_demo -- https://example.com
      ```
  - Without rustls ClientConfig (still prints TemplateID/ALPN/JA3):
    - Windows (PowerShell):
      ```powershell
      cargo run -p htx --example tls_mirror_demo -- https://example.com
      ```
    - Linux/macOS (Bash):
      ```bash
      cargo run -p htx --example tls_mirror_demo -- https://example.com
      ```

- Echo placeholder example (prints a line; will be wired to HTX later):
  - Windows (PowerShell):
    ```powershell
    cargo run -p echo
    ```
  - Linux/macOS (Bash):
    ```bash
    cargo run -p echo
    ```

### Development Workflow
1. Review `qnet-spec/specs/001-qnet/tasks.md` for current priorities
2. Use `qnet-spec/scripts/` for common operations
3. Follow TDD: write tests first, then implement
4. Ensure compliance with `qnet-spec/memory/constitution.md`
5. Submit PRs with clear links to spec requirements

## Contributing

QNet welcomes contributions from developers, security researchers, and protocol designers. Key areas for contribution:

- **Core Implementation**: Rust crates for HTX, crypto, framing
- **Tools**: C library, Go linter, uTLS generator
- **Documentation**: Guides, tutorials, API docs
- **Testing**: Fuzzing, property tests, integration suites
- **Research**: Post-quantum crypto, routing optimizations

### Guidelines
- Follow the constitution principles in `qnet-spec/memory/constitution.md`
- Use the spec kit for all feature proposals
- Maintain high test coverage (target: 80%+)
- Document all public APIs
- Use conventional commits for PRs

**Development Note**: Parts or most of the code in this project have been developed using agentic large language models (LLMs) to accelerate development, ensure modular implementations, and maintain high standards of quality and documentation.

## Bounties

QNet offers bounties for key components to accelerate development:

- **HTX Crate**: Rust implementation of L2 transport
- **Nym Mixnode**: Privacy hop integration
- **C Library**: Cross-platform HTX wrapper
- **Spec Linter**: Compliance checker tool
- **uTLS Generator**: ClientHello template utility

See `qnet-spec/specs/001-qnet/spec.md` for detailed bounty requirements.

## Security

QNet implements multiple layers of security:
- AEAD encryption for all data
- Mutual authentication via Noise XK
- Path validation with SCION signatures
- Post-quantum hybrid cryptography (from 2027)
- Deterministic serialization to prevent parsing attacks
- Anti-correlation measures and cover traffic

For security issues, please email security@qnet.org (placeholder).

## License

QNet is licensed under the MIT License. See LICENSE file for details.

## Roadmap

- **Q1 2025**: Core infrastructure and HTX PoC
- **Q2 2025**: Routing, mesh, and privacy features
- **Q3 2025**: Payments, governance, and tools
- **Q4 2025**: Production-ready release with bounties

## Community

- **Discussions**: GitHub Discussions for general topics
- **Issues**: Bug reports and feature requests
- **Wiki**: Documentation and guides
- **Matrix/Rocket.Chat**: Real-time chat (TBD)

Join us in building the future of decentralized networking!</content>
<parameter name="filePath">p:\GITHUB\qnet\README.md
