# QNet Strategic Roadmap

## Strategy: "Build the Helper, Grow the Mesh"

Our strategy is to deploy a **fully decentralized network** disguised as a simple **browser extension**.

1.  **The Trojan Horse**: Users install a "VPN Extension" for free access.
2.  **The Hidden Node**: The extension installs a local "Helper" service.
3.  **The Mesh**: Every Helper joins the P2P network, strengthening it for everyone.

## Implementation Phases

### Phase 1: Core Infrastructure (Completed) ✅
*Building the engine.*
We have built the foundational Rust crates:
- **`htx`**: The masking transport layer that mimics TLS fingerprints.
- **`core-crypto`**: The cryptographic primitives.
- **`core-framing`**: The secure wire protocol.
- **Catalog System**: The secure update mechanism.

### Phase 2: The Helper Node (Current) 🚧
*Turning the engine into a car.*
We are currently integrating the core crates into the `stealth-browser` binary (the Helper).
- **Goal**: A standalone binary that runs a SOCKS5 proxy and connects to the QNet mesh.
- **Key Tech**: `libp2p` for mesh networking, `htx` for masking.

### Phase 3: User Experience (Current) 🚧
*Giving the car a steering wheel.*
We are building the user-facing components.
- **Browser Extension**: The remote control for the Helper.
- **Installers**: One-click setup for Windows/Linux/macOS.
- **Goal**: Zero-configuration privacy. "It just works."

### Phase 4: Advanced Privacy (Future) 🔮
*Adding armor.*
Once the mesh is live and stable, we enable advanced features:
- **Mixnet**: High-latency routing for extreme anonymity (integrating Nym).
- **Incentives**: Paying relay nodes with Vouchers/Cashu.
- **Governance**: Decentralized protocol upgrades.

## Technology Stack

| Component | Technology | Reason |
|-----------|------------|--------|
| **Core Logic** | **Rust** | Memory safety, performance, async (Tokio). |
| **Mesh Networking** | **libp2p** | Industry standard, modular, robust. |
| **Transport** | **Rustls + Quinn** | Modern TLS 1.3 and QUIC support. |
| **UI** | **WebExtensions** | Cross-browser compatibility (Chrome/Edge/Firefox). |
| **Scripting** | **PowerShell / Bash** | Native automation. |

## Success Metrics
1.  **Indistinguishability**: Traffic MUST look like Microsoft/Google to DPI.
2.  **Usability**: Installation MUST take < 2 minutes.
3.  **Performance**: Latency MUST be acceptable for browsing (Fast Mode) or streaming (Direct Mode).
