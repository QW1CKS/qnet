//! Example: Mesh peer discovery using DiscoveryBehavior
//!
//! This example demonstrates how to use the DiscoveryBehavior to discover
//! other peers in a QNet mesh network using both Kademlia DHT and mDNS.
//!
//! Run with:
//! ```sh
//! cargo run --example mesh_discovery --features with-libp2p
//! ```

use core_mesh::discovery::{BootstrapNode, DiscoveryBehavior};
use libp2p::{identity, Multiaddr, PeerId};
use std::time::Duration;

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("QNet Mesh Discovery Example");
    println!("===========================\n");

    // Generate local peer identity
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());
    println!("Local Peer ID: {}\n", peer_id);

    // Load bootstrap nodes (in production, from signed catalog)
    let bootstrap_nodes = load_bootstrap_nodes_example();
    
    if bootstrap_nodes.is_empty() {
        println!("No bootstrap nodes configured - using mDNS only for LAN discovery");
    } else {
        println!("Bootstrap nodes:");
        for (i, node) in bootstrap_nodes.iter().enumerate() {
            println!("  {}. {} at {}", i + 1, node.peer_id, node.multiaddr);
        }
    }
    println!();

    // Initialize discovery behavior
    println!("Initializing discovery behavior...");
    let mut discovery = DiscoveryBehavior::new(peer_id, bootstrap_nodes).await?;
    println!("Discovery initialized successfully\n");

    // Query initial peer count
    let initial_count = discovery.peer_count();
    println!("Initial peer count: {}", initial_count);

    // Periodic discovery loop
    println!("\nStarting discovery loop (Ctrl+C to exit)...\n");
    
    for iteration in 1..=10 {
        // Wait between queries
        async_std::task::sleep(Duration::from_secs(3)).await;

        // Query current peers
        let peer_count = discovery.peer_count();
        println!("[{}] Peer count: {}", iteration, peer_count);

        // Get detailed peer list
        match discovery.discover_peers().await {
            Ok(peers) => {
                if !peers.is_empty() {
                    println!("    Discovered peers:");
                    for (i, peer) in peers.iter().take(5).enumerate() {
                        println!("      {}. {}", i + 1, peer);
                    }
                    if peers.len() > 5 {
                        println!("      ... and {} more", peers.len() - 5);
                    }
                } else {
                    println!("    No peers discovered yet (waiting for mDNS/DHT)");
                }
            }
            Err(e) => {
                eprintln!("    Error discovering peers: {}", e);
            }
        }
    }

    println!("\nDiscovery example complete");
    Ok(())
}

/// Example bootstrap node loader
///
/// In production, this would load from the signed catalog.
/// For this example, we return mock bootstrap nodes.
fn load_bootstrap_nodes_example() -> Vec<BootstrapNode> {
    // Example: Parse bootstrap nodes from environment or use defaults
    if let Ok(bootstrap_env) = std::env::var("QNET_BOOTSTRAP_NODES") {
        parse_bootstrap_from_env(&bootstrap_env)
    } else {
        // No bootstrap nodes - rely on mDNS for local discovery
        vec![]
    }
}

/// Parse bootstrap nodes from environment variable
///
/// Format: "peer_id@multiaddr,peer_id@multiaddr,..."
/// Example: "12D3Koo...@/ip4/198.51.100.1/tcp/4001"
fn parse_bootstrap_from_env(env_value: &str) -> Vec<BootstrapNode> {
    let mut nodes = Vec::new();
    
    for entry in env_value.split(',') {
        let parts: Vec<&str> = entry.split('@').collect();
        if parts.len() == 2 {
            if let (Ok(peer_id), Ok(multiaddr)) = (
                parts[0].parse::<PeerId>(),
                parts[1].parse::<Multiaddr>(),
            ) {
                nodes.push(BootstrapNode::new(peer_id, multiaddr));
            } else {
                eprintln!("Warning: Failed to parse bootstrap entry: {}", entry);
            }
        }
    }
    
    nodes
}

// Example usage scenarios:
//
// 1. Local network discovery (mDNS only):
//    cargo run --example mesh_discovery --features with-libp2p
//
// 2. With bootstrap nodes (DHT):
//    QNET_BOOTSTRAP_NODES="12D3KooW...@/ip4/198.51.100.1/tcp/4001" \
//    cargo run --example mesh_discovery --features with-libp2p
//
// 3. Multi-node test:
//    Terminal 1: cargo run --example mesh_discovery --features with-libp2p
//    Terminal 2: cargo run --example mesh_discovery --features with-libp2p
//    (Both should discover each other via mDNS on the same LAN)
//
// Note: For full peer discovery, this example would need to run a libp2p
// Swarm event loop with active network transports. The current implementation
// demonstrates the API usage but requires additional Swarm integration for
// live network discovery.
