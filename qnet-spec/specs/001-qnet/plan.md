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

## Infrastructure Strategy

### Bootstrap Infrastructure
**Hybrid Approach: Free + Minimal Cost**

#### Primary: Public libp2p DHT (Free)
- Leverage existing IPFS global infrastructure
- No hosting costs for peer discovery
- Battle-tested reliability (thousands of nodes)
- Implementation: Add public libp2p bootstrap nodes to `hardcoded_seed_nodes()`

#### Secondary: Operator Droplets ($8-18/month)
**Minimal Deployment** (2 droplets @ $4/month = $8/month):
- Droplet 1: NYC (Americas coverage)
- Droplet 2: Amsterdam (Europe/Asia coverage)

**Recommended Deployment** (3 droplets @ $6/month = $18/month):
- Droplet 1: NYC (Americas)
- Droplet 2: Amsterdam (Europe)
- Droplet 3: Singapore (Asia)

**Droplet Specifications**:
- RAM: 512 MB - 1 GB (sufficient for 50-100 concurrent users)
- CPU: 1 vCPU
- Bandwidth: 500-1000 GB/month transfer
- Provider: DigitalOcean, Linode, Vultr, or similar

### Exit Node Strategy
**Professional Exits, Safe Users**

#### Default User Mode: Relay Only
- 99% of users operate in relay-only mode
- Forward encrypted packets (cannot see contents)
- Zero legal liability (just passing through encrypted data)
- No opt-in required - safe by default

#### Operator Exit Nodes
- Same DigitalOcean droplets serve dual purpose:
  1. Backup bootstrap for discovery
  2. Primary exit nodes for actual web requests
- Professional operation with proper abuse policies
- Users protected from exit node legal risks

#### Configuration:
```bash
# Droplet setup (automated)
curl -sSL https://qnet.example.com/deploy-exit.sh | bash

# Serves as:
# - Bootstrap node (helps new users discover peers)
# - Exit node (makes actual web requests)
# - Relay node (forwards traffic through mesh)
```

### Cost Breakdown

| Deployment Size | Droplets | Cost/Month | User Capacity | Coverage |
|----------------|----------|------------|---------------|----------|
| **MVP** | 1 droplet ($4) | $4 | 50-100 users | Single region |
| **Minimal** | 2 droplets ($4 each) | $8 | 100-200 users | Global (Americas + Europe) |
| **Recommended** | 3 droplets ($6 each) | $18 | 200-400 users | Global (All continents) |
| **Growth** | 5-10 droplets | $30-60 | 500-1000 users | High availability |

### Scaling Strategy

**Phase 1: Launch (0-100 users)**
- 1-2 droplets ($4-8/month)
- Public libp2p DHT for discovery
- Operator-run exits only

**Phase 2: Growth (100-1000 users)**
- 3-5 droplets ($18-30/month)
- Community volunteer seeds emerge
- Still operator-run exits (user safety)

**Phase 3: Maturity (1000+ users)**
- 5-10 droplets ($30-60/month)
- Voucher system funds infrastructure (Phase 4)
- Optional community exits (opt-in)
- Network becomes self-sustaining

**Phase 4: Self-Sustaining**
- Vouchers cover all infrastructure costs
- Operator breaks even or profits
- Community contributes volunteer seeds
- Fully decentralized bootstrap + professional exits

