<div align="center">
  <img src="logo.png" alt="QNet Logo" width="400">
</div>

# QNet: Decentralized, Censorship-Resistant Networking

**QNet (QuantaNet)** is a decentralized network protocol stack designed to replace the vulnerable public Internet with a privacy-preserving, self-sovereign alternative. It enables secure, anonymous communication over any IP bearer without relying on centralized authorities or vulnerable DNS systems.

**Why QNet?**  
- **Censorship-Proof**: Route around blocks with stealthy, mixnet-powered tunnels.  
- **Privacy-First**: Traffic mimics normal HTTPS, undetectable by ISPs or governments.  
- **Decentralized**: No central authorities, no DNS vulnerabilities, no single points of failure.  
- **Scalable & Secure**: Built for millions of users with quantum-resistant cryptography.  

Quick links:  
- [Live Task Tracker](qnet-spec/specs/001-qnet/tasks.md)  
- [Specification](qnet-spec/specs/001-qnet/spec.md)  
- [Implementation Plan](qnet-spec/specs/001-qnet/plan.md)  

---

## Table of Contents

- [QNet: Decentralized, Censorship-Resistant Networking](#qnet-decentralized-censorship-resistant-networking)
- [Overview](#overview)
- [Key Features & Benefits](#key-features--benefits)
- [Architecture](#architecture)
- [Technology Stack](#technology-stack)
- [Implementation Phases](#implementation-phases)
- [Progress Tracker](#progress-tracker)
- [For Developers: Toolkit and Framework](#for-developers-toolkit-and-framework)
- [For Users: Ready-to-Use Applications](#for-users-ready-to-use-applications)
- [Getting Started](#getting-started)
- [Physical Testing](#physical-testing)
- [Contributing](#contributing)
- [Bounties](#bounties)
- [Security](#security)
- [License](#license)
- [Roadmap](#roadmap)
- [Community](#community)

---

## Overview

QNet is a decentralized, censorship-resistant network designed to replace the public Internet. It provides strong user privacy, decentralized operation, and resistance to censorship through layered architecture and advanced cryptography.

### User Stories
- **As a user**, I want to connect to QNet using standard internet access so that I can access decentralized services without relying on centralized infrastructure.
- **As a developer**, I want to build applications on QNet using self-certifying identities so that users can verify service authenticity without external authorities.
- **As a privacy-conscious user**, I want my traffic to be routed through mixnodes so that observers cannot correlate my communications.

### Layered Architecture
QNet's innovative 7-layer architecture ensures seamless, secure connectivity:

- **L0 Access Media**: Agnostic to any IP bearer (fiber, 5G, satellite, LoRa, etc.)
- **L1 Path Selection & Routing**: Secure routing with SCION + HTX-tunnelled transitions
- **L2 Cover Transport**: HTX over TCP-443/QUIC-443 with origin-mirrored TLS
- **L3 Overlay Mesh**: libp2p-v2 object relay for peer-to-peer connections
- **L4 Privacy Hops**: Nym mixnet for optional anonymity
- **L5 Naming & Trust**: Self-certifying IDs + 3-chain alias ledger
- **L6 Payments**: Federated Cashu + Lightning for micro-payments
- **L7 Applications**: Web-layer replacement services

### Cryptography & Security
- **As a security engineer**, I want all communications encrypted with ChaCha20-Poly1305, Ed25519 signatures, X25519 DH, and HKDF-SHA256 so that data is protected against eavesdropping and tampering.
- **As a future-proof system**, I want post-quantum hybrid X25519-Kyber768 from 2027 so that the network remains secure against quantum attacks.

---

## Key Features & Benefits

### Why Choose QNet?
- **Origin-Mirrored TLS Handshakes**: Auto-calibrates to mimic real websites perfectly—your traffic blends in.
- **Noise XK Inner Channels**: Mutual authentication with post-quantum hybrids for unbreakable security.
- **SCION Packet Headers**: Validates paths to prevent routing attacks.
- **Mixnode Selection**: Uses BeaconSet randomness for diverse, uncorrelated hops.
- **Alias Ledger**: 2-of-3 finality for decentralized naming.
- **Voucher-Based Payments**: Micro-payments via Lightning integration.
- **Anti-Correlation Measures**: Cover traffic and fallback mechanisms.
- **Reproducible Builds**: SLSA provenance for trust.

### Real-World Impact
- **Journalists & Activists**: Access blocked sites without detection.
- **Developers**: Build censorship-resistant apps effortlessly.
- **Everyday Users**: Browse freely, no matter where you are.
- **Governments & ISPs**: Can't block what they can't see!

QNet isn't just better—it's a paradigm shift. Say goodbye to surveillance and hello to true digital freedom.

---

## Architecture

QNet's modular design ensures scalability and maintainability:

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

### Layer Mapping
- **L0**: OS/system libraries handle access media.
- **L1**: `routing/` with SCION and HTX tunneling.
- **L2**: `htx/` with Noise XK and framing.
- **L3**: `mesh/` with libp2p transports.
- **L4**: `mixnode/` with Nym integration.
- **L5**: `naming/` with alias ledger.
- **L6**: Voucher handling in core.
- **L7**: Application examples.

---

## Technology Stack

### Core Languages
- **Rust**: Primary for HTX, mixnode, and core networking—memory-safe, performant, async-ready.
- **C**: For cross-platform C library bounty.
- **Go**: For spec-compliance linter CLI tools.

### Key Libraries & Frameworks
- **tokio**: Async runtime for high-performance concurrency.
- **ring**: Cryptography (ChaCha20-Poly1305, Ed25519, X25519, HKDF).
- **libp2p**: P2P overlay mesh and peer discovery.
- **rustls**: TLS handling with origin mirroring.
- **quinn**: QUIC for UDP-443 support.
- **scion-rust**: SCION protocol for secure routing.
- **nym-sdk**: Mixnet for privacy hops.
- **sled**: Embedded database for state.
- **serde**: Serialization for CBOR and more.

### Infrastructure
- **Docker**: Reproducible builds and deployment.
- **Linux**: Primary OS for servers.
- **GitHub Actions**: CI/CD with SLSA provenance.

---

## Implementation Phases

### Phase 1: Core Infrastructure (High Priority)
- Rust workspace setup with tokio and ring.
- Crypto primitives (ChaCha20, Ed25519, etc.).
- L2 framing and AEAD handling.
- Noise XK handshake.
- SCION packet structures.

### Phase 2: HTX Proof-of-Concept (High Priority)
- TLS origin mirroring with rustls.
- QUIC support.
- Inner channel establishment.
- Frame multiplexing and flow control.
- Fuzzing for 80% coverage.

### Phase 3: Routing & Mesh (Medium Priority)
- SCION path construction and validation.
- HTX tunneling for non-SCION links.
- libp2p integration for peer discovery.
- Bootstrap with rotating DHT and adaptive PoW.

### Phase 4: Privacy & Naming (Medium Priority)
- Mixnode selection with BeaconSet.
- Nym mixnet integration.
- Self-certifying ID implementation.
- Alias ledger with 2-of-3 finality.

### Phase 5: Payments & Governance (Low Priority)
- Voucher format and validation.
- Lightning integration.
- Governance scoring and voting.

### Phase 6: Tools & Compliance (Medium Priority)
- C library implementation.
- Go spec linter.
- uTLS template generator.
- SLSA provenance pipeline.
- Stealth browser application.

---

## Progress Tracker

Track our journey to QNet's full realization:

- [x] T1.1: Project Structure Setup
- [x] T1.2: Crypto Primitives Implementation
- [x] T1.3: L2 Frame Handling
- [x] T1.4: Noise XK Handshake
- [x] T1.5: Deterministic CBOR & TemplateID
- [x] T2.1: TLS Origin Mirroring
- [x] T2.2: Inner Channel Establishment
- [x] T2.3: Frame Multiplexing
- [x] T2.4: HTX Crate API
- [x] T2.5: Fuzzing and Testing
- [x] T2.6: L2 Frame Semantics & KEY_UPDATE Behavior
- [x] T3.1: SCION Packet Structures
- [x] T3.2: HTX Tunneling
- [x] T3.3: libp2p Integration
- [x] T3.4: Bootstrap Discovery
- [x] T3.5: Translation Layer (v1.1 Interop)
- [x] T4.1: Mixnode Selection
- [x] T4.2: Nym Mixnet Integration
- [x] T4.3: Self-Certifying IDs
- [x] T4.4: Alias Ledger
- [x] T5.1: Voucher System
- [x] T5.2: Governance Scoring
- [x] T6.1: C Library Implementation
- [x] T6.2: Go Spec Linter
- [x] T6.3: uTLS Template Generator
- [x] T6.4: SLSA Provenance
- [ ] T6.5: Compliance Test Harness
- [x] T6.6: Performance Optimization
- [ ] T6.7: Stealth Browser Application

For full details, see [qnet-spec/specs/001-qnet/tasks.md](qnet-spec/specs/001-qnet/tasks.md).

## Goals

- **Censorship Resistance**: Operate without single points of failure
- **User Privacy**: Strong anonymity and metadata minimization
- **Decentralization**: No central authorities for naming or routing
- **Interoperability**: Compatible with existing internet infrastructure
- **Scalability**: Support millions of users and services
- **Security**: Quantum-resistant cryptography and formal verification
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

## For Developers: Toolkit and Framework

QNet is primarily a toolkit and framework for developers to build private, censorship-resistant networks and applications. The core components are in the `crates/` directory:

- **Core Crates**: `core-crypto`, `core-framing`, `htx`, etc. - Low-level primitives for building QNet-based apps.
- **Examples**: `examples/` - Sample code showing how to integrate QNet into your applications.
- **Libraries**: `c-lib/` - C bindings for non-Rust projects.

To get started as a developer:
```bash
# Clone the repo
git clone https://github.com/QW1CKS/qnet.git
cd qnet

# Build the workspace
cargo build --workspace

# Run an example
cargo run --example echo
```

See the [Specification](qnet-spec/specs/001-qnet/spec.md) for API details and integration guides.

## For Users: Ready-to-Use Applications

While QNet is a developer toolkit, we also provide ready-to-use applications for day-to-day users. These are built on top of the QNet protocol stack and are designed for ease of use without requiring development knowledge.

- **Stealth Browser**: A browser application that uses QNet to browse the web anonymously, mimicking normal HTTPS traffic to evade ISP tracking and censorship. Located in `apps/`.

To use the stealth browser:
```bash
# Build the browser (when implemented)
cargo build --release --bin stealth-browser
./target/release/stealth-browser
```

These applications are separate from the core toolkit to keep the repository organized: developers focus on `crates/` and `examples/`, while users can download pre-built binaries or build the apps from `apps/`.

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

### Windows (MSVC): build the C library and echo example

Use the Visual Studio x64 Native Tools Command Prompt (or Developer PowerShell) so cl.exe and the Windows SDK are on PATH.

Windows PowerShell (recommended):

```powershell
# From the repo root
cargo build -p qnet_c -r

# Build the cdylib and compile the C example with MSVC
Set-Location -Path 'crates/c-lib'
./build-windows-msvc.ps1 -Configuration Release

# Run the example (ensure qnet_c.dll is alongside echo.exe)
Set-Location -Path 'examples'
./echo.exe
```

Native Tools Command Prompt (cmd):

```cmd
REM Change drive and directory if needed
cd /d <repo-path>
cargo build -p qnet_c -r

cd crates\c-lib
powershell -ExecutionPolicy Bypass -File .\build-windows-msvc.ps1 -Configuration Release

cd examples
echo.exe
```

Troubleshooting:
- "cl.exe not found": open the "x64 Native Tools Command Prompt for VS" and retry.
- "kernel32.lib not found": install a Windows SDK via VS Build Tools and use the Developer shell.
- "bcrypt.lib not found": this also comes from the Windows 10/11 SDK. Use the Visual Studio Installer to add a Windows SDK (Desktop C++), then reopen the x64 Developer shell so LIB includes `...\Windows Kits\10\Lib\<ver>\um\x64`. If already installed, ensure you're in the 64-bit Developer shell and not a plain PowerShell.
- "cannot cd to repo": in cmd use `cd /d P:\GITHUB\qnet` (the `/d` switches drives), or `pushd P:\GITHUB\qnet`.
- "server accept failed" in echo example: This was a sequencing issue in the C code. The client must open the stream before the server accepts it. The example has been fixed to run the client open/write first, then server accept/echo.
- "fatal error C1083: Cannot open include file: 'stdio.h'": The MSVC environment variables are not set. In PowerShell, run these commands to set them manually (adjust paths if your SDK/MSVC versions differ):

```powershell
$env:VCToolsInstallDir = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207"
$sdk = "10.0.26100.0"
$sdkRoot = "C:\Program Files (x86)\Windows Kits\10"
$vcInc = Join-Path $env:VCToolsInstallDir "include"
$vcLib = Join-Path $env:VCToolsInstallDir "lib\x64"
$ucrtInc = Join-Path $sdkRoot "Include\$sdk\ucrt"
$umInc = Join-Path $sdkRoot "Include\$sdk\um"
$shared = Join-Path $sdkRoot "Include\$sdk\shared"
$ucrtLib = Join-Path $sdkRoot "Lib\$sdk\ucrt\x64"
$umLib = Join-Path $sdkRoot "Lib\$sdk\um\x64"
$env:INCLUDE = "$vcInc;$ucrtInc;$umInc;$shared"
$env:LIB = "$vcLib;$ucrtLib;$umLib"
```

Then retry the build and run commands.

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

### Go Spec Linter

QNet includes a Go-based CLI tool for validating implementations against the specification. It checks compliance with L2 framing, TemplateID, KEY_UPDATE, and BN-Ticket headers, and generates SBOMs for security tracking.

#### Installation
```bash
cd linter
go mod download
go build -o qnet-lint ./cmd/qnet-lint
```

#### Usage
```bash
# Validate QNet implementation
./qnet-lint validate /path/to/qnet/project

# Generate SBOM
### Generate SBOM
```bash
./qnet-lint sbom /path/to/qnet/project
```

Note: SBOM generation requires the external Syft tool. Install it separately for full functionality.

#### Features
- **L2 Framing Validation**: Ensures AEAD protection and length checks
- **TemplateID Validation**: Verifies deterministic CBOR and SHA-256 computation
- **KEY_UPDATE Validation**: Checks 3-frame overlap and nonce lifecycle
- **BN-Ticket Validation**: Validates 256-byte header compliance
- **SBOM Generation**: Uses Syft to create Software Bill of Materials
- **CI Integration**: Automated via GitHub Actions workflow

The linter runs automatically in CI and provides clear error messages for non-compliant code.

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

For security issues, please submit an issue or contact the maintainers directly.

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

Join us in building the future of decentralized networking!

## Physical Testing

QNet includes hands-on testing tasks using physical setups (e.g., multiple computers) to validate functionality, performance, and stealth. These tests progress from basic connectivity to advanced scenarios like censorship bypass and multi-hop routing.

For the full detailed table with inputs, outputs, and tracking, see [qnet-spec/specs/001-qnet/tasks.md](qnet-spec/specs/001-qnet/tasks.md#physical-testing-tasks).

| Task Description | Inputs | Expected Outputs | Status |
|------------------|--------|------------------|--------|
| Network Setup and Connectivity | Connect computers via local network; assign static IPs. | Successful ping between devices. | Pending |
| QNet Daemon Build and Launch | Build and run daemons on each computer with local config. | Daemons start; logs show peer discovery. | Pending |
| Basic Peer Discovery | Enable gossipsub; check logs for connections. | Mutual peer discovery without timeouts. | Pending |
| Simple HTTP Tunnel Test | Run client/server daemons; curl via tunnel. | HTTP response received. | Pending |
| Frame Encoding/Decoding Validation | Send test frames; inspect with Wireshark. | Frames decode correctly; no corruption. | Pending |
| Noise Handshake Verification | Initiate handshake; log secrets. | Handshake completes; keys match. | Pending |
| Stealth Mode Packet Mimicry | Enable TLS mimicry; capture traffic. | Packets look like standard HTTPS. | Pending |
| Latency Benchmarking | Run iperf over tunnel. | Latency <50ms; throughput >100Mbps. | Pending |
| Censorship Bypass Simulation | Block IP; route via QNet. | Traffic bypasses block. | Pending |
| Browser Extension Prototype | Install extension; route .qnet domains. | Requests tunneled successfully. | Pending |
| Stealth Browser Application Test | Build full stealth browser with QNet; browse censored sites. | Auto-connects; traffic mimics HTTPS; content loads. | Pending |
| Performance Under Load | Simulate high traffic. | No crashes; stable performance. | Pending |
| Edge Case: Network Disruption | Disconnect/reconnect during tunnel. | Automatic recovery; no data loss. | Pending |
| Advanced Stealth: Decoy Routing | Configure decoy domains; route through them. | Real destination hidden. | Pending |

Update the full table in tasks.md after each test run.
