# Core Mix

[![Crates.io](https://img.shields.io/crates/v/core-mix.svg)](https://crates.io/crates/core-mix)
[![Documentation](https://docs.rs/core-mix/badge.svg)](https://docs.rs/core-mix)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Privacy-preserving mix network for QNet** - Anonymous communication through layered encryption, traffic mixing, and cryptographic unlinkability.

## Overview

The `core-mix` crate implements QNet's mixnet privacy layer:

- **Layered Encryption**: Onion-style message encryption
- **Traffic Mixing**: Message batching and reordering
- **Cryptographic Unlinkability**: Sender/receiver anonymity
- **Mix Node Operation**: Decentralized mix network
- **Traffic Analysis Resistance**: Constant-rate padding
- **Forward Secrecy**: Ephemeral key rotation

## Features

- ✅ **Anonymous**: Sender and receiver unlinkability
- ✅ **Unobservable**: Traffic analysis resistance
- ✅ **Decentralized**: No central mixing authority
- ✅ **Scalable**: High-throughput mixing
- ✅ **Secure**: Cryptographically secure anonymity

## Quick Start

```rust
use core_mix::{MixClient, MixNode, MixConfig, SphinxPacket};

// Create mix client
let client_config = MixConfig {
    mixnet_id: "qnet-mixnet".to_string(),
    ..Default::default()
};
let mut client = MixClient::new(client_config)?;

// Create mix node
let node_config = MixConfig {
    listen_addr: "0.0.0.0:8081".parse()?,
    ..Default::default()
};
let mut mix_node = MixNode::new(node_config)?;
mix_node.start().await?;

// Send anonymous message
let destination = "destination-peer-id".parse()?;
let message = b"Anonymous message through mixnet";
let sphinx_packet = client.create_packet(destination, message)?;

client.send_packet(sphinx_packet).await?;
```

## API Reference

### Mix Configuration

```rust
pub struct MixConfig {
    pub mixnet_id: String,             // Mixnet identifier
    pub listen_addr: SocketAddr,       // Node listen address
    pub batch_size: usize,             // Messages per batch
    pub batch_timeout: Duration,       // Maximum batch wait time
    pub padding_size: usize,           // Traffic padding size
    pub enable_loopix: bool,           // Enable Loopix-style mixing
}
```

### Mix Client

```rust
pub struct MixClient {
    config: MixConfig,
    route_cache: RouteCache,
    crypto_state: CryptoState,
}

impl MixClient {
    pub fn new(config: MixConfig) -> Result<Self, Error>
    pub fn create_packet(&mut self, destination: PeerId, payload: &[u8]) -> Result<SphinxPacket, Error>
    pub async fn send_packet(&mut self, packet: SphinxPacket) -> Result<(), Error>
    pub async fn receive_message(&mut self) -> Result<Vec<u8>, Error>
}
```

### Mix Node

```rust
pub struct MixNode {
    config: MixConfig,
    message_pool: MessagePool,
    crypto_keys: EphemeralKeys,
}

impl MixNode {
    pub fn new(config: MixConfig) -> Result<Self, Error>
    pub async fn start(&mut self) -> Result<(), Error>
    pub async fn process_packet(&mut self, packet: SphinxPacket) -> Result<(), Error>
    pub async fn flush_batch(&mut self) -> Result<(), Error>
}
```

### Sphinx Packet

```rust
pub struct SphinxPacket {
    pub header: SphinxHeader,
    pub body: Vec<u8>,
}

pub struct SphinxHeader {
    pub version: u8,
    pub alpha: [u8; 32],      // Group element
    pub beta: [u8; 16],       // Encrypted routing info
    pub gamma: [u8; 32],      // MAC
}
```

## Mixnet Architecture

### Sphinx Protocol

**Onion Routing:**
1. **Layered Encryption**: Each hop peels one encryption layer
2. **Perfect Forward Secrecy**: Ephemeral keys per message
3. **Route Hiding**: Destination revealed only to final hop
4. **Length Hiding**: Fixed-size packets with padding

**Packet Structure:**
```
Sphinx Packet:
├── Header (192 bytes)
│   ├── Version (1 byte)
│   ├── Alpha (32 bytes)     # Group element
│   ├── Beta (16 bytes)      # Encrypted routing info
│   └── Gamma (32 bytes)     # MAC
└── Body (Variable, padded)
    ├── Payload (Encrypted)
    └── Padding (Random)
```

### Mixing Strategies

#### Pool Mixing
- **Batch Collection**: Accumulate messages in pools
- **Random Ordering**: Shuffle message order
- **Timed Release**: Periodic batch flushing
- **Size Hiding**: Fixed batch sizes

#### Loopix Mixing
- **Decoy Traffic**: Artificial cover traffic
- **Exponential Delays**: Poisson-distributed timing
- **Multi-Hop**: Three-hop route structure
- **Dummy Messages**: Constant-rate padding

#### Threshold Mixing
- **Minimum Batch**: Wait for minimum messages
- **Timeout Release**: Maximum wait time
- **Adaptive Batching**: Dynamic batch sizing

## Security Properties

### Anonymity Guarantees

**Sender Anonymity:**
- **Unlinkability**: Sender identity hidden from observers
- **Unobservability**: Traffic patterns concealed
- **Resistance to Timing Attacks**: Constant-rate transmission
- **Resistance to Volume Attacks**: Traffic padding

**Receiver Anonymity:**
- **Destination Hiding**: Final destination concealed
- **Route Ambiguity**: Multiple possible routes
- **Cover Traffic**: Artificial traffic generation

### Cryptographic Security

**Forward Secrecy:**
- **Ephemeral Keys**: Fresh keys per session
- **Key Rotation**: Periodic key updates
- **Perfect Forward Secrecy**: Compromised keys don't affect past messages

**Authentication:**
- **Message Integrity**: HMAC verification
- **Node Authentication**: Cryptographic node verification
- **Replay Protection**: Nonce-based replay prevention

## Performance

**Throughput Metrics:**

| Configuration | Messages/sec | Latency |
|----------------|--------------|---------|
| Single hop | ~50K | ~10ms |
| 3-hop route | ~15K | ~50ms |
| 5-hop route | ~8K | ~100ms |
| With mixing | ~5K | ~200ms |

**Resource Usage:**
- Memory: ~100MB for active mixing pools
- CPU: ~10% for cryptographic operations
- Bandwidth: ~20% overhead for padding/headers

## Advanced Usage

### Custom Mixing Strategy

```rust
use core_mix::{MixNode, MixingStrategy};

struct CustomMixer;

impl MixingStrategy for CustomMixer {
    async fn mix_batch(&self, messages: Vec<SphinxPacket>) -> Vec<SphinxPacket> {
        // Custom mixing logic
        custom_shuffle_and_delay(messages).await
    }
}

let mix_node = MixNode::with_strategy(config, CustomMixer);
```

### Route Selection

```rust
// Select privacy-optimized route
let route = client.select_route(destination, PrivacyLevel::High)?;
let packet = client.create_packet_for_route(route, message)?;
```

### Cover Traffic Generation

```rust
// Generate decoy traffic
mix_node.enable_cover_traffic(CoverTrafficConfig {
    rate: 100,  // packets per second
    size_distribution: Exponential { lambda: 0.1 },
})?;
```

## Error Handling

```rust
use core_mix::Error;

match result {
    Ok(_) => log!("Mix operation successful"),
    Err(Error::InvalidPacket) => log!("Malformed Sphinx packet"),
    Err(Error::RouteFailure) => log!("Failed to establish route"),
    Err(Error::MixingTimeout) => log!("Batch mixing timeout"),
    Err(Error::AuthenticationFailed) => log!("Node authentication failed"),
    Err(Error::ReplayDetected) => log!("Replay attack detected"),
    Err(_) => log!("Unknown mix error"),
}
```

## Testing

Run the test suite:

```bash
cargo test
```

Run anonymity tests:

```bash
cargo test --features anonymity
```

Run benchmarks:

```bash
cargo bench
```

## Integration

Add to your `Cargo.toml`:

```toml
[dependencies]
core-mix = { path = "../crates/core-mix" }
```

For external projects:

```toml
[dependencies]
core-mix = "0.1"
```

## Architecture

```
core-mix/
├── src/
│   ├── lib.rs           # Main library interface
│   ├── client.rs        # Mix client implementation
│   ├── node.rs          # Mix node implementation
│   ├── sphinx.rs        # Sphinx protocol
│   ├── mixing.rs        # Message mixing strategies
│   ├── crypto.rs        # Cryptographic operations
│   ├── config.rs        # Configuration structures
│   └── error.rs         # Error types
├── tests/               # Unit tests
├── benches/             # Performance benchmarks
└── examples/            # Usage examples
```

## Related Crates

- **`core-routing`**: Route selection for mix paths
- **`core-crypto`**: Cryptographic primitives
- **`core-mesh`**: Peer-to-peer connectivity

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