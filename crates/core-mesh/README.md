# Core Mesh

[![Crates.io](https://img.shields.io/crates/v/core-mesh.svg)](https://crates.io/crates/core-mesh)
[![Documentation](https://docs.rs/core-mesh/badge.svg)](https://docs.rs/core-mesh)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Decentralized mesh networking for QNet** - Peer-to-peer connectivity with automatic routing, NAT traversal, and resilient overlay networks.

## Overview

The `core-mesh` crate provides QNet's mesh networking layer, enabling:

- **Peer Discovery**: Automatic peer detection and connection
- **NAT Traversal**: Hole punching and relay fallback
- **Overlay Routing**: Multi-hop message delivery
- **Connection Management**: Reliable transport connections
- **Network Resilience**: Automatic failover and recovery
- **Traffic Optimization**: Path selection and load balancing

## Features

- ✅ **Decentralized**: No central coordination required
- ✅ **NAT Friendly**: Automatic NAT traversal techniques
- ✅ **Self-Healing**: Automatic route discovery and repair
- ✅ **Scalable**: Efficient routing for large networks
- ✅ **Secure**: Encrypted peer-to-peer connections

## Quick Start

```rust
use core_mesh::{MeshNode, MeshConfig, PeerId};
use std::net::SocketAddr;

// Create mesh node configuration
let config = MeshConfig {
    listen_addr: "0.0.0.0:0".parse()?,
    bootstrap_peers: vec![
        "bootstrap.qnet.io:8080".parse()?,
    ],
    ..Default::default()
};

// Create and start mesh node
let mut node = MeshNode::new(config)?;
node.start().await?;

// Connect to a peer
let peer_id = PeerId::from_public_key(peer_public_key);
let connection = node.connect(peer_id).await?;

// Send a message
connection.send(b"Hello, mesh peer!").await?;

// Receive messages
while let Some(message) = connection.receive().await {
    println!("Received: {:?}", message);
}
```

## API Reference

### Mesh Configuration

```rust
pub struct MeshConfig {
    pub listen_addr: SocketAddr,           // Local listen address
    pub bootstrap_peers: Vec<SocketAddr>,  // Initial peer list
    pub max_connections: usize,            // Connection limit
    pub enable_nat_traversal: bool,        // Enable NAT punching
    pub enable_relay: bool,                // Enable relay fallback
    pub heartbeat_interval: Duration,      // Keepalive interval
}
```

### Mesh Node

```rust
pub struct MeshNode {
    config: MeshConfig,
    peer_id: PeerId,
    connections: HashMap<PeerId, Connection>,
}

impl MeshNode {
    pub async fn new(config: MeshConfig) -> Result<Self, Error>
    pub async fn start(&mut self) -> Result<(), Error>
    pub async fn connect(&mut self, peer_id: PeerId) -> Result<Connection, Error>
    pub async fn broadcast(&mut self, message: &[u8]) -> Result<(), Error>
    pub fn get_peers(&self) -> Vec<PeerId>
}
```

### Peer Identity

```rust
pub struct PeerId([u8; 32]);

impl PeerId {
    pub fn from_public_key(key: &[u8; 32]) -> Self
    pub fn to_bytes(&self) -> [u8; 32]
    pub fn distance(&self, other: &PeerId) -> Distance
}
```

### Connections

```rust
pub struct Connection {
    peer_id: PeerId,
    state: ConnectionState,
    send_queue: mpsc::Sender<Vec<u8>>,
    recv_queue: mpsc::Receiver<Vec<u8>>,
}

impl Connection {
    pub async fn send(&mut self, data: &[u8]) -> Result<(), Error>
    pub async fn receive(&mut self) -> Option<Vec<u8>>
    pub async fn close(&mut self) -> Result<(), Error>
}
```

## Network Architecture

### Peer Discovery

**Bootstrap Process:**
1. Connect to bootstrap peers
2. Exchange peer lists
3. Iterative peer discovery
4. Maintain routing table

**Discovery Methods:**
- **DHT**: Distributed hash table for peer lookup
- **Gossip**: Epidemic peer list propagation
- **MDNS**: Local network peer discovery
- **Bootstrap**: Initial peer list seeding

### NAT Traversal

**Techniques Used:**
- **STUN**: Session Traversal Utilities for NAT
- **TURN**: Traversal Using Relays around NAT
- **ICE**: Interactive Connectivity Establishment
- **Hole Punching**: UDP/TCP hole punching

**Fallback Strategy:**
1. Direct connection attempt
2. NAT traversal techniques
3. Relay through intermediate peer
4. Give up (temporary failure)

### Routing

**Routing Table:**
- **Kademlia-style DHT**: XOR distance-based routing
- **Backup Routes**: Multiple paths per destination
- **Route Metrics**: Latency, bandwidth, reliability
- **Route Updates**: Gossip-based route propagation

**Message Delivery:**
- **Direct**: Single-hop to destination
- **Relayed**: Multi-hop through intermediate peers
- **Broadcast**: Flooding to all connected peers
- **Multicast**: Selective forwarding to peer groups

## Security Considerations

### Authentication
- **Peer Verification**: Cryptographic peer authentication
- **Connection Encryption**: All connections encrypted
- **Message Integrity**: HMAC verification on all messages

### Access Control
- **Peer Whitelisting**: Restrict connections to known peers
- **Rate Limiting**: Prevent DoS attacks
- **Connection Limits**: Maximum concurrent connections

### Privacy
- **Traffic Analysis**: Constant-rate padding
- **Metadata Protection**: Encrypted peer lists
- **Anonymity**: Mixnet integration for anonymity

## Performance

**Network Metrics:**

| Metric | Value |
|--------|-------|
| Connection Latency | < 100ms |
| Message Throughput | > 100 Mbps |
| Peer Discovery Time | < 5 seconds |
| Route Convergence | < 30 seconds |

**Resource Usage:**
- Memory: ~50MB for 1000 peers
- CPU: ~5% for active routing
- Bandwidth: ~10KB/s maintenance traffic

## Advanced Usage

### Custom Peer Discovery

```rust
use core_mesh::{MeshNode, DiscoveryService};

struct CustomDiscovery;

impl DiscoveryService for CustomDiscovery {
    async fn discover_peers(&self) -> Vec<PeerId> {
        // Custom peer discovery logic
        get_peers_from_dns().await
    }
}

let node = MeshNode::with_discovery(config, CustomDiscovery);
```

### Connection Monitoring

```rust
// Monitor connection health
node.on_connection(|peer_id, state| {
    match state {
        ConnectionState::Connected => log!("Connected to {}", peer_id),
        ConnectionState::Disconnected => log!("Disconnected from {}", peer_id),
        ConnectionState::Failed => log!("Connection failed to {}", peer_id),
    }
});
```

### Message Routing

```rust
// Custom routing logic
node.set_router(|message, peers| {
    // Select best peer for message delivery
    select_optimal_peer(message, peers)
});
```

## Error Handling

```rust
use core_mesh::Error;

match result {
    Ok(_) => log!("Operation successful"),
    Err(Error::ConnectionFailed) => log!("Failed to connect to peer"),
    Err(Error::PeerNotFound) => log!("Peer not found in network"),
    Err(Error::NatTraversalFailed) => log!("NAT traversal failed"),
    Err(Error::AuthenticationFailed) => log!("Peer authentication failed"),
    Err(_) => log!("Unknown mesh error"),
}
```

## Testing

Run the test suite:

```bash
cargo test
```

Run network simulation tests:

```bash
cargo test --features simulation
```

Run benchmarks:

```bash
cargo bench
```

## Integration

Add to your `Cargo.toml`:

```toml
[dependencies]
core-mesh = { path = "../crates/core-mesh" }
```

For external projects:

```toml
[dependencies]
core-mesh = "0.1"
```

## Architecture

```
core-mesh/
├── src/
│   ├── lib.rs           # Main library interface
│   ├── node.rs          # Mesh node implementation
│   ├── connection.rs    # Peer connections
│   ├── discovery.rs     # Peer discovery services
│   ├── routing.rs       # Message routing
│   ├── nat.rs           # NAT traversal
│   ├── config.rs        # Configuration structures
│   └── error.rs         # Error types
├── tests/               # Unit tests
├── benches/             # Performance benchmarks
└── examples/            # Usage examples
```

## Related Crates

- **`core-routing`**: Higher-level routing decisions
- **`core-mix`**: Mixnet privacy layer
- **`htx`**: Secure tunnel establishment

## Contributing

See the main [Contributing Guide](../docs/CONTRIBUTING.md) for development setup and contribution guidelines.

### Development Requirements

- Follow [AI Guardrail](../qnet-spec/memory/ai-guardrail.md)
- Meet [Testing Rules](../qnet-spec/memory/testing-rules.md)
- Include `AI-Guardrail: PASS` and `Testing-Rules: PASS` in commits

## License

Licensed under the MIT License. See [LICENSE](../../LICENSE) for details.

## Security

If you discover a security vulnerability, please see our [Security Policy](../../SECURITY.md).

---

*Part of the [QNet](https://github.com/QW1CKS/qnet) project - Building the future of decentralized networking.*