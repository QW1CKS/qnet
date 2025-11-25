# QNet System Architecture

## High-Level Components

The QNet system consists of three main components:

1.  **Browser Extension (`apps/extension`)**:
    - **Role**: User Interface & Proxy Controller.
    - **Tech**: WebExtensions API (JS/HTML/CSS).
    - **Communication**: Native Messaging to the Helper.

2.  **Helper Service (`apps/stealth-browser`)**:
    - **Role**: The core logic engine. Acts as a SOCKS5 server, P2P node, and HTX client.
    - **Tech**: Rust (Tokio, Hyper, Libp2p).
    - **Communication**:
        - **Input**: SOCKS5 from Browser, Native Messaging commands.
        - **Output**: Encrypted HTX traffic to the Mesh.

3.  **The Mesh (`crates/core-mesh`)**:
    - **Role**: The global network of peers.
    - **Tech**: Libp2p (Noise, Yamux, Kademlia DHT).

## Data Flow

### 1. User Request
User types `amazon.com` in the browser.

### 2. Extension Interception
The extension (via PAC script or Proxy API) redirects the request to the local SOCKS5 proxy: `127.0.0.1:1088`.

### 3. Helper Processing
The Helper receives the SOCKS5 request for `amazon.com`.
1.  **Catalog Lookup**: Checks the signed catalog for a valid **Decoy Node** (Entry Node).
2.  **Path Selection**: Determines the route (Direct to Decoy, or via Mesh Peers).
3.  **HTX Encapsulation**:
    - Generates a ClientHello matching the Decoy's fingerprint (e.g., Microsoft).
    - Establishes a TLS connection to the Decoy.
    - Performs an inner Noise XK handshake.

### 4. Mesh Routing
The encrypted packet travels through the mesh.
- **Entry Node**: Receives the HTX packet, decrypts the inner layer.
- **Relay Nodes**: Forward the packet based on the circuit ID.
- **Exit Node**: Makes the actual TCP connection to `amazon.com`.

### 5. Response
The response follows the reverse path, encrypted at each step, until it reaches the Helper, which unwraps it and sends it back to the browser via SOCKS5.

## Directory Structure

### `apps/` (User Facing)
- `stealth-browser/`: The Helper binary (Rust).
- `extension/`: The Browser Extension (JS).

### `crates/` (Core Logic)
- `htx/`: The masking transport layer.
- `core-crypto/`: Cryptographic primitives.
- `core-framing/`: Wire protocol definitions.
- `core-mesh/`: P2P networking logic.
- `catalog-signer/`: Tooling for the update system.

## Security Model

- **Trust**: We do NOT trust the ISP. We do NOT trust individual peers with metadata (onion routing).
- **Updates**: All updates (catalogs, binaries) must be signed by the developer keys.
- **Fingerprinting**: We assume the ISP performs Deep Packet Inspection (DPI) and matches TLS fingerprints against known browsers.

---

## Mesh Peer Discovery (Phase 2.1)

The QNet mesh network uses a two-tiered discovery system to enable Helpers to find each other automatically:

### Discovery Mechanisms

1. **Kademlia DHT (Wide-Area Discovery)**
   - Structured peer-to-peer distributed hash table
   - Bootstrap nodes loaded from signed catalog (catalog-first priority)
   - Enables discovery across the internet
   - Periodic refresh every 5 minutes maintains routing table freshness
   - Fallback to hardcoded seeds only if catalog unavailable

2. **mDNS (Local Network Discovery)**
   - Multicast DNS for LAN peer discovery
   - Zero-configuration discovery on local networks
   - No bootstrap infrastructure required
   - Automatic peer announcement and listening

### Discovery Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ Helper Startup                                                   │
│                                                                   │
│ 1. Generate Ed25519 peer identity                                │
│ 2. Load bootstrap nodes (catalog → seeds fallback)               │
│ 3. Initialize DiscoveryBehavior (Kademlia + mDNS)                │
│ 4. Spawn dedicated async-std discovery thread                    │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│ Discovery Thread (async-std runtime, isolated from tokio)        │
│                                                                   │
│ Every 5 seconds:                                                 │
│   • Query peer_count() from Kademlia routing table               │
│   • Update AppState.mesh_peer_count (AtomicU32)                  │
│   • Log discovered peers (state-transition markers)              │
│                                                                   │
│ Concurrent Events:                                               │
│   • mDNS announces local peer presence                           │
│   • mDNS listens for other peers on LAN                          │
│   • Kademlia DHT processes bootstrap connections                 │
│   • Kademlia DHT maintains routing table                         │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│ Status API Integration                                           │
│                                                                   │
│ GET /status returns:                                             │
│   {                                                              │
│     "peers_online": <atomic_read>,  // Updated every 5s          │
│     "state": "connected",                                        │
│     "socks_addr": "127.0.0.1:1088",                              │
│     ...                                                          │
│   }                                                              │
└─────────────────────────────────────────────────────────────────┘
```

### Implementation Details

**Module**: `crates/core-mesh/src/discovery.rs`

**Key Types**:
- `BootstrapNode`: Peer ID + Multiaddr for DHT seeding
- `DiscoveryBehavior`: libp2p NetworkBehaviour combining Kademlia + mDNS
- `load_bootstrap_nodes()`: Catalog-first loader (returns empty without catalog)

**Helper Integration** (`apps/stealth-browser/src/main.rs`):
- `spawn_mesh_discovery()`: Dedicated std::thread running async-std runtime
- `AppState.mesh_peer_count`: Arc<AtomicU32> for thread-safe peer count
- Runtime bridging: async-std (libp2p) isolated from tokio (Helper main)

**Status Exposure**:
- `build_status_json()` reads `mesh_peer_count.load(Ordering::Relaxed)`
- Populates `peers_online` field in status response
- Browser extension can display live peer count

### Catalog-First Priority

Per QNet architecture, peer discovery prioritizes the signed catalog:
1. Attempt to load bootstrap nodes from verified catalog
2. Only fall back to hardcoded seeds if catalog unavailable/invalid
3. Log `catalog-first:` warnings when falling back to seeds

### Testing

**Unit Tests** (`crates/core-mesh/src/discovery.rs`):
- Bootstrap node creation and validation
- DiscoveryBehavior initialization
- peer_count() and discover_peers() API contracts

**Integration Tests** (`tests/integration/tests/mesh_discovery.rs`):
- Multi-node discovery scenarios (structure validation)
- Bootstrap DHT configuration
- Peer count consistency checks
- API performance contracts (non-blocking guarantees)

**Physical Tests** (per `qnet-spec/docs/physical-testing.md`):
- Actual multi-machine network discovery
- Real mDNS broadcast verification
- Cross-internet DHT peer finding
- Production-like mesh behavior

**Example** (`crates/core-mesh/examples/mesh_discovery.rs`):
- Demonstrates DiscoveryBehavior usage
- Periodic peer count queries
- Bootstrap node configuration
- Environment-based setup

### Limitations

Current implementation provides API and structure but requires libp2p Swarm event loop integration for full live network discovery:
- DiscoveryBehavior defines NetworkBehaviour trait
- Actual peer connections require Swarm runner with transports
- mDNS broadcasts need active network I/O
- DHT routing requires connection establishment

Future phases will integrate Swarm runtime for complete mesh functionality.

---

## Bootstrap Infrastructure (Phase 2.5)

QNet employs a hybrid bootstrap strategy that balances decentralization with reliability while minimizing operator costs.

### Primary: Public libp2p DHT (Free Infrastructure)

**Global Peer Discovery Without QNet Servers**

QNet leverages the existing IPFS/libp2p distributed hash table for bootstrap:

- **Zero Infrastructure Cost**: Uses public IPFS bootstrap nodes maintained by the global IPFS community
- **Battle-Tested Reliability**: Thousands of IPFS nodes worldwide provide redundant discovery
- **No Single Point of Failure**: Decentralized DHT ensures network remains accessible
- **Implementation**: `public_libp2p_seeds()` returns well-known IPFS bootstrap multiaddrs

**Bootstrap Multiaddrs** (from IPFS project):
```
/dnsaddr/bootstrap.libp2p.io
/dnsaddr/ipfs.io
/ip4/104.131.131.82/tcp/4001/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ
```

### Secondary: Operator Seed Nodes (Minimal VPS)

**Backup Bootstrap + Primary Exit Nodes**

Small DigitalOcean droplets ($4-6/month) serve dual purpose:
1. Secondary bootstrap if public DHT unavailable
2. Primary exit nodes (see Exit Node Architecture below)

**Recommended Deployment**:
- 2 droplets @ $4/month = $8/month (minimal)
- 3 droplets @ $6/month = $18/month (global coverage)

**Regions**: NYC (Americas), Amsterdam (Europe), Singapore (Asia)

### Catalog-Based Updates

Bootstrap node lists can be dynamically updated via signed catalogs:
- Operator can add new droplets without code changes
- Community volunteers can contribute seed nodes
- Smooth migration path as network grows
- Catalog verification ensures trust

---

## Exit Node Architecture (Phase 2.5)

QNet protects users from legal liability through a three-tier exit node model.

### The Exit Node Problem

**Risk**: If home users act as exits, their IP makes actual web requests, exposing them to legal liability for others' traffic.

**Solution**: Professional operator-run exit nodes, with users defaulting to safe relay-only mode.

### Tier 1: User Helpers (Relay-Only Mode)

**Default Configuration: 99% of Network**

- **Role**: Forward encrypted packets through the mesh
- **Legal Protection**: Cannot see packet contents (end-to-end encrypted via HTX)
- **Defense**: "I was relaying encrypted data, similar to an ISP routing traffic"
- **Risk**: Zero (never decrypt or make actual requests)
- **Configuration**: `--relay-only` (default, no opt-in needed)

**What Users Do**:
```
User A Traffic → [Encrypted Packet] → User B Helper → [Still Encrypted] → Exit Node
```

User B never sees destination or content - just encrypted bytes.

### Tier 2: Operator Exit Nodes (Primary Exits)

**Professional Operation on Small VPS**

- **Infrastructure**: DigitalOcean droplets ($4-6/month each)
- **Role**: Decrypt HTX packets and make actual HTTP/HTTPS requests
- **Legal**: Proper abuse policies, logging, terms of service
- **Reliability**: 99.9% uptime, dedicated bandwidth, multiple regions
- **Cost Efficiency**: 2-3 droplets serve 200-400 users globally

**What Operator Exits Do**:
```
User A Traffic → Relay Nodes → Operator Exit → [Decrypted] → amazon.com
```

Operator exit sees destination and makes request (legal liability managed properly).

**Exit Policy Controls**:
- Bandwidth limiting per user (prevent abuse)
- Protocol filtering (HTTP/HTTPS only, block Tor/BitTorrent)
- Rate limiting and abuse detection
- Logging for law enforcement compliance

### Tier 3: Volunteer Exits (Opt-In Only)

**Advanced Users on VPS**

- **Target**: Experienced users who understand legal risks
- **Requirement**: Explicit opt-in with legal disclaimers
- **Recommendation**: Run on VPS, not home connections
- **Configuration**: `--exit-node` flag + legal acceptance
- **Policy**: Granular exit policy controls (destinations, protocols, bandwidth)

**Warning Display**:
```
⚠️  EXIT NODE WARNING
By enabling exit functionality, your IP will make web requests for other users.
You may receive legal notices or abuse complaints. Ensure you understand your local laws.

[ ] I understand the risks and accept legal liability
    [Cancel]  [Enable Exit Node]
```

### Cost & Economics

| Deployment | Users | Cost/Month | Exit Capacity |
|-----------|-------|------------|---------------|
| MVP (1 droplet) | 50-100 | $4 | 500 GB/month |
| Minimal (2 droplets) | 100-200 | $8 | 1 TB/month |
| Recommended (3 droplets) | 200-400 | $18 | 2 TB/month |
| Growth (5-10 droplets) | 500-1000 | $30-60 | 5-10 TB/month |

**Scaling**: Add $4-6/month per region as network grows. Voucher system (Phase 4) funds infrastructure.

### Implementation

**Helper Configuration** (`apps/stealth-browser/src/main.rs`):
```rust
pub struct HelperMode {
    relay: bool,        // Default: true (always forward encrypted packets)
    exit: bool,         // Default: false (opt-in only for liability)
    bootstrap: bool,    // Default: false (only for operator seeds)
}
```

**CLI Flags**:
```bash
# Default user mode (safe, no liability)
stealth-browser --relay-only

# Operator droplet (exit + bootstrap)
stealth-browser --exit-node --bootstrap

# Advanced volunteer exit (VPS recommended)
stealth-browser --exit-node --exit-policy=strict.json
```

**Exit Policy** (`exit-policy.json`):
```json
{
  "max_bandwidth_mbps": 100,
  "allowed_protocols": ["http", "https"],
  "blocked_destinations": ["torrent-tracker.example.com"],
  "rate_limit_per_user_mbps": 5,
  "abuse_detection": {
    "max_connections_per_minute": 60,
    "blocked_on_abuse_for_minutes": 30
  }
}
```

### Legal Protection Strategy

**For Users (Relay-Only)**:
- Encrypted forwarding only (cannot see content)
- Similar legal status to ISPs and VPNs
- No decryption = no liability for content

**For Operator Exits**:
- Professional operation with abuse policies
- Proper logging for law enforcement
- Terms of service
 clearly state traffic relay nature
- Located in jurisdictions with safe harbor laws (NL, SE, IS)

**For Volunteer Exits**:
- Clear opt-in with legal disclaimers
- Run on VPS in safe jurisdictions, not home
- Exit policy controls limit exposure
- Tor-style "I'm running a Tor exit" legal templates

---

