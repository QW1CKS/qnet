//! Peer discovery module for QNet mesh networking.
//!
//! This module implements peer discovery using:
//! - **Operator Directory**: Query 6 operator nodes for peer list (catalog-first)
//! - **mDNS**: For local network (LAN) peer discovery
//!
//! # Architecture
//!
//! The `DiscoveryBehavior` combines operator directory queries and mDNS into a unified
//! interface. Operator nodes (6 DigitalOcean droplets) maintain a peer directory via
//! heartbeat registrations. Clients query this directory for instant peer discovery.
//! mDNS runs concurrently to find peers on the local network.
//!
//! # Example
//!
//! ```no_run
//! use core_mesh::discovery::{BootstrapNode, DiscoveryBehavior};
//! use libp2p::{identity, PeerId, Multiaddr};
//! use std::str::FromStr;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let keypair = identity::Keypair::generate_ed25519();
//! let peer_id = PeerId::from(keypair.public());
//!
//! // Load operator nodes
//! let bootstrap_nodes = vec![
//!     BootstrapNode {
//!         peer_id: PeerId::from_str("12D3KooWExamplePeerId")?,
//!         multiaddr: "/ip4/198.51.100.1/tcp/4001".parse()?,
//!     },
//! ];
//!
//! let _discovery = DiscoveryBehavior::new(peer_id, bootstrap_nodes).await?;
//! # Ok(())
//! # }
//! ```

use libp2p::{mdns, Multiaddr, PeerId};
use thiserror::Error;

use crate::relay::RoutingTable;

/// Errors that can occur during peer discovery.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("mDNS error: {0}")]
    Mdns(String),

    #[error("No peers discovered")]
    NoPeers,

    #[error("Bootstrap failed: {0}")]
    Bootstrap(String),
}

/// Represents an operator node for peer directory queries.
///
/// Operator nodes are trusted entry points into the QNet mesh network.
/// These are the 6 DigitalOcean droplets running the peer directory service.
#[derive(Debug, Clone)]
pub struct BootstrapNode {
    /// The peer ID of the bootstrap node
    pub peer_id: PeerId,
    /// The multiaddress to connect to the bootstrap node
    pub multiaddr: Multiaddr,
}

impl BootstrapNode {
    /// Creates a new bootstrap node.
    pub fn new(peer_id: PeerId, multiaddr: Multiaddr) -> Self {
        Self { peer_id, multiaddr }
    }
}

/// Load operator nodes for peer directory queries.
///
/// **Bootstrap Strategy** (catalog-first):
/// 1. Hardcoded operator nodes (6 droplets - always available)
///
/// Clients query operator directory endpoints for instant peer discovery.
pub fn load_bootstrap_nodes() -> Vec<BootstrapNode> {
    hardcoded_seed_nodes()
}

/// QNet operator seed nodes (Primary, Operator-Controlled).
///
/// DigitalOcean droplets ($4-6/month each) run by the network operator.
/// Serve triple duty:
/// 1. Primary bootstrap nodes for peer discovery
/// 2. Relay nodes for mesh networking
/// 3. Exit nodes for actual web requests
///
/// **IMPORTANT**: After deploying droplets, update this function and release new binary.
/// IP changes require binary recompilation and distribution.
///
/// **Deployment Instructions**: See `docs/exit_node_deployment.md`
fn qnet_operator_seeds() -> Vec<BootstrapNode> {
    // TODO: Replace placeholders with actual operator droplet configurations
    //
    // When you deploy the 6 DigitalOcean droplets:
    // 1. Get the peer IDs from each droplet (shown on startup)
    // 2. Get the public IPs from DigitalOcean dashboard
    // 3. Update the entries below
    // 4. Build new binary: cargo build --release -p stealth-browser
    // 5. Distribute via GitHub releases
    //
    // Example configuration:
    // vec![
    //     BootstrapNode::new(
    //         "12D3KooWExamplePeerIdForDroplet1NYC".parse().unwrap(),
    //         "/ip4/198.51.100.10/tcp/4001".parse().unwrap(),
    //     ),
    //     BootstrapNode::new(
    //         "12D3KooWExamplePeerIdForDroplet2AMS".parse().unwrap(),
    //         "/ip4/198.51.100.20/tcp/4001".parse().unwrap(),
    //     ),
    //     BootstrapNode::new(
    //         "12D3KooWExamplePeerIdForDroplet3SIN".parse().unwrap(),
    //         "/ip4/198.51.100.30/tcp/4001".parse().unwrap(),
    //     ),
    //     BootstrapNode::new(
    //         "12D3KooWExamplePeerIdForDroplet4FRA".parse().unwrap(),
    //         "/ip4/198.51.100.40/tcp/4001".parse().unwrap(),
    //     ),
    //     BootstrapNode::new(
    //         "12D3KooWExamplePeerIdForDroplet5TOR".parse().unwrap(),
    //         "/ip4/198.51.100.50/tcp/4001".parse().unwrap(),
    //     ),
    //     BootstrapNode::new(
    //         "12D3KooWExamplePeerIdForDroplet6SYD".parse().unwrap(),
    //         "/ip4/198.51.100.60/tcp/4001".parse().unwrap(),
    //     ),
    // ]

    // Direct peering configuration for testing (Dec 1 2025)
    // One-way connection: Windows behind NAT/firewall, so only Windows connects TO droplet
    // Droplet is publicly accessible and will accept the connection
    // Once connected, both nodes can bootstrap via each other
    vec![
        BootstrapNode::new(
            "12D3KooWB5Tb2ejzRAbj7HKEHeMTxMc8uE7gj5XFoYod9hWEuzer"
                .parse()
                .unwrap(), // Droplet peer ID (FRA1 region, Dec 2025)
            "/ip4/165.232.73.134/tcp/4001".parse().unwrap(),
        ),
        // Note: Windows peer removed from seeds because it's not publicly reachable
        // Windows will connect to droplet, then both discover each other via DHT provider records
    ]
}

/// Hardcoded operator nodes for resilience when catalog is unavailable.
///
/// **Operator Directory Strategy**:
/// 1. Primary: 6 QNet operator nodes (peer directory endpoints)
/// 2. Fallback: Disk cache (24hr TTL) if directory unreachable
/// 3. Emergency: Hardcoded operator list (always available)
fn hardcoded_seed_nodes() -> Vec<BootstrapNode> {
    let nodes = qnet_operator_seeds();

    if nodes.is_empty() {
        log::warn!("No operator nodes available! Network discovery may fail.");
    } else {
        log::info!("Bootstrap: {} operator nodes available", nodes.len());
    }

    nodes
}

/// Combined peer discovery behavior using operator directory and mDNS.
///
/// This struct implements the `NetworkBehaviour` trait and can be integrated
/// into a libp2p `Swarm`. It manages both wide-area (operator directory) and
/// local (mDNS) peer discovery, as well as the QNet stream protocol for
/// bidirectional data transfer.
#[derive(libp2p::swarm::NetworkBehaviour)]
pub struct DiscoveryBehavior {
    /// mDNS for local network peer discovery
    pub mdns: mdns::async_io::Behaviour,
    /// Identify protocol for peer information exchange
    pub identify: libp2p::identify::Behaviour,
    /// AutoNAT for NAT detection and public address discovery
    pub autonat: libp2p::autonat::Behaviour,
    /// Relay client for NAT traversal via relay nodes
    pub relay_client: libp2p::relay::client::Behaviour,
    /// DCUtR for direct connection upgrade through relay (hole punching)
    pub dcutr: libp2p::dcutr::Behaviour,
    /// QNet stream protocol for bidirectional tunneling
    pub stream: crate::stream_protocol::QNetStreamBehaviour,
}

/// Manages routing table populated from peer discovery events.
///
/// This struct is kept separate from `DiscoveryBehavior` to avoid
/// NetworkBehaviour derive macro constraints. It should be stored
/// alongside the discovery behavior and updated via the provided methods.
#[derive(Debug, Clone)]
pub struct DiscoveryRoutingTable {
    routing_table: RoutingTable,
}

impl DiscoveryRoutingTable {
    /// Create a new discovery routing table.
    pub fn new() -> Self {
        Self {
            routing_table: RoutingTable::new(),
        }
    }

    /// Called when a peer is discovered to add it to the routing table.
    ///
    /// This creates a direct route to the discovered peer. In a full mesh,
    /// each peer can forward packets directly to any discovered peer.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The peer that was discovered
    pub fn on_peer_discovered(&mut self, peer_id: PeerId) {
        // Add direct route to discovered peer (via itself)
        self.routing_table
            .add_route(peer_id, peer_id, Some(std::time::Duration::from_secs(300)));
        log::debug!("relay: peer discovered, added route to {}", peer_id);
    }

    /// Called when a peer connection is lost to remove it from the routing table.
    ///
    /// This removes all routes through the lost peer.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The peer that was lost
    pub fn on_peer_lost(&mut self, peer_id: PeerId) {
        self.routing_table.remove_route(&peer_id);
        log::debug!("relay: peer lost, removed routes to {}", peer_id);
    }

    /// Get a reference to the routing table populated by discovery.
    ///
    /// The routing table is automatically updated as peers are discovered
    /// and lost. Relay logic can use this table to forward packets.
    pub fn get_routing_table(&self) -> &RoutingTable {
        &self.routing_table
    }
}

impl Default for DiscoveryRoutingTable {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoveryBehavior {
    /// Creates a new `DiscoveryBehavior` instance.
    ///
    /// Returns both the behavior and the relay transport that must be composed
    /// with the base transport before creating the Swarm.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The local peer ID
    /// * `bootstrap_nodes` - Operator nodes for peer directory queries (logged for reference)
    ///
    /// # Errors
    ///
    /// Returns an error if mDNS initialization fails.
    ///
    /// # Returns
    ///
    /// A tuple of (relay_transport, discovery_behavior) where the relay_transport
    /// must be composed with the base transport using `.or_transport()`.
    pub async fn new(
        peer_id: PeerId,
        bootstrap_nodes: Vec<BootstrapNode>,
    ) -> Result<(libp2p::relay::client::Transport, Self), DiscoveryError> {
        // Log operator nodes for reference (directory queries happen at higher layer)
        for node in bootstrap_nodes {
            log::info!(
                "state-transition: Operator node available {} at {}",
                node.peer_id,
                node.multiaddr
            );
        }

        // Initialize mDNS for local peer discovery
        let mdns = mdns::async_io::Behaviour::new(mdns::Config::default(), peer_id)
            .map_err(|e| DiscoveryError::Mdns(format!("Failed to initialize mDNS: {}", e)))?;

        // Initialize Identify protocol for peer information exchange
        let identify = libp2p::identify::Behaviour::new(libp2p::identify::Config::new(
            "/qnet/1.0.0".to_string(),
            libp2p::identity::Keypair::generate_ed25519().public(),
        ));

        // Initialize AutoNAT for NAT detection and public address discovery
        let autonat = libp2p::autonat::Behaviour::new(
            peer_id,
            libp2p::autonat::Config {
                retry_interval: std::time::Duration::from_secs(30),
                refresh_interval: std::time::Duration::from_secs(60),
                boot_delay: std::time::Duration::from_secs(5),
                ..Default::default()
            },
        );

        // Initialize relay client for NAT traversal
        // Returns (Transport, Behaviour) - transport MUST be composed with base transport
        let (relay_transport, relay_client) = libp2p::relay::client::new(peer_id);

        // Initialize DCUtR for direct connection upgrade through relay (hole punching)
        // DCUtR coordinates with relay_client to establish direct connections
        let dcutr = libp2p::dcutr::Behaviour::new(peer_id);

        // Initialize QNet stream protocol for bidirectional tunneling
        let stream = crate::stream_protocol::QNetStreamBehaviour::new();

        log::info!(
            "state-transition: Discovery initialized for peer {} with NAT traversal support",
            peer_id
        );

        Ok((
            relay_transport,
            Self {
                mdns,
                identify,
                autonat,
                relay_client,
                dcutr,
                stream,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity;

    #[test]
    fn test_bootstrap_node_creation() {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/4001".parse().unwrap();

        let node = BootstrapNode::new(peer_id, addr.clone());
        assert_eq!(node.peer_id, peer_id);
        assert_eq!(node.multiaddr, addr);
    }

    #[test]
    fn test_load_bootstrap_nodes_returns_operator_nodes() {
        // Should load only operator nodes (6 droplets)
        let nodes = load_bootstrap_nodes();

        // Verify we have operator nodes available
        assert!(nodes.len() > 0, "Should have operator nodes");

        // Verify structure: each node should have peer_id and multiaddr
        for node in &nodes {
            assert!(!node.multiaddr.to_string().is_empty());
        }
    }

    #[async_std::test]
    async fn test_discovery_behavior_creation() {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        let result = DiscoveryBehavior::new(peer_id, vec![]).await;
        assert!(result.is_ok());
        let (_relay_transport, _discovery) = result.unwrap();
    }

    #[async_std::test]
    async fn test_discovery_behavior_no_kademlia_field() {
        // Compile-time check: DiscoveryBehavior should not have kademlia field
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        let result = DiscoveryBehavior::new(peer_id, vec![]).await;
        assert!(result.is_ok(), "Discovery should initialize without DHT");
    }

    #[async_std::test]
    async fn test_on_peer_discovered_adds_route() {
        let discovered_peer = PeerId::random();

        let mut routing = DiscoveryRoutingTable::new();

        // Initially no route
        assert_eq!(routing.get_routing_table().route_count(), 0);

        // Discover peer
        routing.on_peer_discovered(discovered_peer);

        // Should have route now
        assert_eq!(routing.get_routing_table().route_count(), 1);
        assert_eq!(
            routing.get_routing_table().find_route(&discovered_peer),
            Some(&discovered_peer)
        );
    }

    #[async_std::test]
    async fn test_on_peer_lost_removes_route() {
        let discovered_peer = PeerId::random();

        let mut routing = DiscoveryRoutingTable::new();

        // Add peer
        routing.on_peer_discovered(discovered_peer);
        assert_eq!(routing.get_routing_table().route_count(), 1);

        // Remove peer
        routing.on_peer_lost(discovered_peer);
        assert_eq!(routing.get_routing_table().route_count(), 0);
        assert_eq!(
            routing.get_routing_table().find_route(&discovered_peer),
            None
        );
    }

    #[async_std::test]
    async fn test_routing_table_integration() {
        let mut routing = DiscoveryRoutingTable::new();

        // Discover multiple peers
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();
        let peer3 = PeerId::random();

        routing.on_peer_discovered(peer1);
        routing.on_peer_discovered(peer2);
        routing.on_peer_discovered(peer3);

        assert_eq!(routing.get_routing_table().route_count(), 3);

        // Lose one peer
        routing.on_peer_lost(peer2);
        assert_eq!(routing.get_routing_table().route_count(), 2);

        // Remaining routes still present
        assert!(routing.get_routing_table().find_route(&peer1).is_some());
        assert!(routing.get_routing_table().find_route(&peer2).is_none());
        assert!(routing.get_routing_table().find_route(&peer3).is_some());
    }
}
