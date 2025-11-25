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

