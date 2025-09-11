# Core Routing

[![Crates.io](https://img.shields.io/crates/v/core-routing.svg)](https://crates.io/crates/core-routing)
[![Documentation](https://docs.rs/core-routing/badge.svg)](https://docs.rs/core-routing)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../../LICENSE)

**Intelligent routing engine for QNet** - Multi-path routing, traffic optimization, QoS management, and adaptive path selection for optimal network performance.

## Overview

The `core-routing` crate provides QNet's intelligent routing layer:

- **Multi-Path Routing**: Concurrent path utilization
- **Traffic Optimization**: Load balancing and congestion control
- **Quality of Service**: Priority-based routing
- **Adaptive Routing**: Dynamic path selection
- **Network Monitoring**: Performance metrics and analytics
- **Path Diversity**: Resilience through multiple routes

## Features

- ✅ **Multi-Path**: Utilizes multiple network paths simultaneously
- ✅ **QoS Aware**: Quality of service routing
- ✅ **Adaptive**: Learns and adapts to network conditions
- ✅ **Resilient**: Automatic failover and recovery
- ✅ **Optimized**: Performance-aware routing decisions

## Quick Start

```rust
use core_routing::{Router, RouteConfig, RouteMetrics};
use std::net::IpAddr;

// Create routing configuration
let config = RouteConfig {
    max_paths: 4,
    enable_load_balancing: true,
    enable_qos: true,
    ..Default::default()
};

// Create router instance
let mut router = Router::new(config)?;

// Add network paths
let path1 = NetworkPath::new("10.0.0.1".parse()?, 8080);
let path2 = NetworkPath::new("10.0.0.2".parse()?, 8080);
router.add_path(path1)?;
router.add_path(path2)?;

// Route a packet
let destination = "192.168.1.100".parse()?;
let packet = create_packet(destination, b"Hello, world!");
let route = router.select_route(&packet)?;

// Send via selected route
send_via_route(route, packet).await?;
```

## API Reference

### Router Configuration

```rust
pub struct RouteConfig {
    pub max_paths: usize,              // Maximum concurrent paths
    pub enable_load_balancing: bool,   // Enable load balancing
    pub enable_qos: bool,              // Enable QoS routing
    pub path_selection_algorithm: PathSelectionAlgorithm,
    pub congestion_control: CongestionControlAlgorithm,
    pub heartbeat_interval: Duration,  // Path health checking
}
```

### Router Core

```rust
pub struct Router {
    config: RouteConfig,
    paths: Vec<NetworkPath>,
    metrics: RouteMetrics,
    path_selector: Box<dyn PathSelector>,
}

impl Router {
    pub fn new(config: RouteConfig) -> Result<Self, Error>
    pub fn add_path(&mut self, path: NetworkPath) -> Result<(), Error>
    pub fn remove_path(&mut self, path_id: PathId) -> Result<(), Error>
    pub fn select_route(&mut self, packet: &Packet) -> Result<Route, Error>
    pub fn update_metrics(&mut self, path_id: PathId, metrics: PathMetrics)
    pub fn get_route_metrics(&self) -> &RouteMetrics
}
```

### Network Paths

```rust
pub struct NetworkPath {
    id: PathId,
    endpoint: SocketAddr,
    protocol: Protocol,
    metrics: PathMetrics,
}

pub struct PathMetrics {
    pub latency: Duration,
    pub bandwidth: u64,        // bytes per second
    pub packet_loss: f64,      // percentage
    pub jitter: Duration,
    pub reliability: f64,      // 0.0 to 1.0
}
```

### Route Selection

```rust
pub enum PathSelectionAlgorithm {
    RoundRobin,
    LeastLatency,
    HighestBandwidth,
    LowestLoss,
    QoSAware(QoSProfile),
    Adaptive(AdaptiveConfig),
}

pub struct QoSProfile {
    pub priority: Priority,
    pub max_latency: Duration,
    pub min_bandwidth: u64,
    pub max_packet_loss: f64,
}
```

## Routing Algorithms

### Adaptive Routing

**Algorithm Features:**
- **Machine Learning**: Learns optimal paths over time
- **Real-time Adaptation**: Responds to network changes
- **Predictive Routing**: Anticipates network conditions
- **Feedback Loop**: Uses performance feedback for optimization

**Learning Process:**
1. Monitor path performance metrics
2. Update routing weights based on performance
3. Predict future performance using historical data
4. Select optimal path combination

### Multi-Path Routing

**Path Utilization:**
- **Equal Cost Multi-Path (ECMP)**: Load balancing across equal paths
- **Unequal Cost Multi-Path**: Weighted load balancing
- **Packet Spraying**: Per-packet load balancing
- **Flow-based Splitting**: Per-flow path assignment

**Benefits:**
- **Increased Throughput**: Aggregate bandwidth utilization
- **Improved Reliability**: Redundancy and failover
- **Reduced Latency**: Parallel transmission
- **Load Distribution**: Balanced network utilization

### QoS-Aware Routing

**QoS Classes:**
```rust
pub enum Priority {
    Background = 0,
    BestEffort = 1,
    Video = 2,
    Voice = 3,
    Critical = 4,
}
```

**QoS Routing Logic:**
- **Priority Queuing**: Higher priority traffic first
- **Bandwidth Reservation**: Guaranteed bandwidth allocation
- **Latency Guarantees**: Maximum latency constraints
- **Loss Guarantees**: Maximum packet loss tolerance

## Performance Monitoring

### Metrics Collection

```rust
// Collect path metrics
let metrics = router.get_path_metrics(path_id)?;
println!("Latency: {}ms", metrics.latency.as_millis());
println!("Bandwidth: {}Mbps", metrics.bandwidth / 1_000_000);
println!("Packet Loss: {:.2}%", metrics.packet_loss * 100.0);
```

### Performance Analytics

**Key Metrics:**
- **Path Latency**: Round-trip time measurements
- **Bandwidth Utilization**: Available vs. used bandwidth
- **Packet Loss Rate**: Percentage of lost packets
- **Jitter**: Latency variation
- **Reliability Score**: Overall path reliability

**Analytics Features:**
- **Trend Analysis**: Performance trends over time
- **Predictive Modeling**: Future performance prediction
- **Anomaly Detection**: Unusual performance patterns
- **Capacity Planning**: Network scaling recommendations

## Advanced Usage

### Custom Path Selection

```rust
use core_routing::{Router, PathSelector, PathSelection};

struct CustomSelector;

impl PathSelector for CustomSelector {
    fn select_path(&self, paths: &[NetworkPath], packet: &Packet) -> PathSelection {
        // Custom path selection logic
        select_best_path_for_packet(paths, packet)
    }
}

let router = Router::with_selector(config, CustomSelector);
```

### QoS Configuration

```rust
// Configure QoS profiles
let qos_config = QoSConfig {
    profiles: vec![
        QoSProfile {
            priority: Priority::Voice,
            max_latency: Duration::from_millis(100),
            min_bandwidth: 64_000,  // 64Kbps
            max_packet_loss: 0.01,  // 1%
        },
        QoSProfile {
            priority: Priority::Video,
            max_latency: Duration::from_millis(200),
            min_bandwidth: 1_000_000,  // 1Mbps
            max_packet_loss: 0.05,   // 5%
        },
    ],
};

router.set_qos_config(qos_config)?;
```

### Route Optimization

```rust
// Optimize routing based on current conditions
router.optimize_routes().await?;

// Get optimization recommendations
let recommendations = router.get_optimization_recommendations()?;
for rec in recommendations {
    println!("Recommendation: {}", rec.description);
    apply_recommendation(rec).await?;
}
```

## Error Handling

```rust
use core_routing::Error;

match result {
    Ok(route) => send_via_route(route),
    Err(Error::NoAvailablePaths) => log!("No paths available for routing"),
    Err(Error::PathUnreachable) => log!("Destination unreachable"),
    Err(Error::QoSViolation) => log!("QoS requirements cannot be met"),
    Err(Error::CongestionDetected) => log!("Network congestion detected"),
    Err(_) => log!("Unknown routing error"),
}
```

## Performance

**Routing Performance:**

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Path Selection | ~5 µs | ~200K ops/s |
| Metrics Update | ~10 µs | ~100K ops/s |
| Route Optimization | ~500 µs | ~2K ops/s |

**Scalability:**
- **Paths Supported**: Up to 64 concurrent paths
- **Routes/sec**: > 100K routing decisions per second
- **Memory Usage**: ~10MB for 1000 paths
- **CPU Usage**: ~2% for active routing

## Testing

Run the test suite:

```bash
cargo test
```

Run routing simulation tests:

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
core-routing = { path = "../crates/core-routing" }
```

For external projects:

```toml
[dependencies]
core-routing = "0.1"
```

## Architecture

```
core-routing/
├── src/
│   ├── lib.rs           # Main library interface
│   ├── router.rs        # Main routing engine
│   ├── path.rs          # Network path management
│   ├── metrics.rs       # Performance metrics
│   ├── selector.rs      # Path selection algorithms
│   ├── qos.rs           # Quality of service
│   ├── config.rs        # Configuration structures
│   └── error.rs         # Error types
├── tests/               # Unit tests
├── benches/             # Performance benchmarks
└── examples/            # Usage examples
```

## Related Crates

- **`core-mesh`**: Provides network paths
- **`core-governance`**: Routing policy decisions
- **`core-mix`**: Privacy-preserving routing

## Contributing

See the main [Contributing Guide](../CONTRIBUTING.md) for development setup and contribution guidelines.

### Development Requirements

- Follow [AI Guardrail](../../memory/ai-guardrail.md)
- Meet [Testing Rules](../../memory/testing-rules.md)
- Include `AI-Guardrail: PASS` and `Testing-Rules: PASS` in commits

## License

Licensed under the MIT License. See [LICENSE](../../LICENSE) for details.

## Security

If you discover a security vulnerability, please see our [Security Policy](../../SECURITY.md).

---

*Part of the [QNet](https://github.com/QW1CKS/qnet) project - Building the future of decentralized networking.*