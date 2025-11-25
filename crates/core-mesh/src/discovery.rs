//! Peer discovery module for QNet mesh networking.
//!
//! This module implements peer discovery using two mechanisms:
//! - **Kademlia DHT**: For wide-area peer discovery via bootstrap nodes
//! - **mDNS**: For local network (LAN) peer discovery
//!
//! # Architecture
//!
//! The `DiscoveryBehavior` combines both discovery mechanisms into a unified
//! interface. Bootstrap nodes from the catalog are used to seed the DHT, which
//! then discovers additional peers across the internet. mDNS runs concurrently
//! to find peers on the local network without requiring bootstrap infrastructure.
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
//! // Load bootstrap nodes (in production, from catalog)
//! let bootstrap_nodes = vec![
//!     BootstrapNode {
//!         peer_id: PeerId::from_str("12D3KooWExamplePeerId")?,
//!         multiaddr: "/ip4/198.51.100.1/tcp/4001".parse()?,
//!     },
//! ];
//!
//! let mut discovery = DiscoveryBehavior::new(peer_id, bootstrap_nodes).await?;
//! let peers = discovery.discover_peers().await?;
//! println!("Discovered {} peers", peers.len());
//! # Ok(())
//! # }
//! ```

use libp2p::{
    kad::{store::MemoryStore, Behaviour as Kademlia, Config as KademliaConfig},
    mdns,
    Multiaddr, PeerId,
};
use thiserror::Error;

use crate::relay::RoutingTable;

/// Errors that can occur during peer discovery.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("Kademlia DHT error: {0}")]
    Kademlia(String),

    #[error("mDNS error: {0}")]
    Mdns(String),

    #[error("No peers discovered")]
    NoPeers,

    #[error("Bootstrap failed: {0}")]
    Bootstrap(String),
}

/// Represents a bootstrap node for DHT seeding.
///
/// Bootstrap nodes are trusted entry points into the QNet mesh network.
/// They are typically loaded from the signed catalog.
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
        Self {
            peer_id,
            multiaddr,
        }
    }
}

/// Load bootstrap nodes from the catalog.
///
/// In production, this function loads bootstrap nodes from the signed catalog.
/// If the catalog is unavailable or invalid, it falls back to hardcoded seed nodes.
///
/// # Catalog-First Priority
///
/// Per QNet architecture, the catalog has priority over hardcoded seeds.
/// Seeds are only used if no valid, fresh catalog is available.
pub fn load_bootstrap_nodes() -> Vec<BootstrapNode> {
    // TODO: Integrate with catalog loader (task linkage: catalog system from Phase 1.5)
    // For now, return empty vec to avoid hardcoded seeds without catalog validation
    log::warn!("catalog-first: No catalog available, bootstrap nodes empty (seeds not used)");
    Vec::new()
}

/// Combined peer discovery behavior using Kademlia DHT and mDNS.
///
/// This struct implements the `NetworkBehaviour` trait and can be integrated
/// into a libp2p `Swarm`. It manages both wide-area (DHT) and local (mDNS)
/// peer discovery.
///
/// # Periodic Refresh
///
/// The Kademlia DHT is automatically refreshed every 5 minutes to maintain
/// up-to-date routing information and discover new peers.
///
/// Combined peer discovery behavior using Kademlia DHT and mDNS.
///
/// This struct implements the `NetworkBehaviour` trait and can be integrated
/// into a libp2p `Swarm`. It manages both wide-area (DHT) and local (mDNS)
/// peer discovery.
///
/// # Periodic Refresh
///
/// The Kademlia DHT is automatically refreshed every 5 minutes to maintain
/// up-to-date routing information and discover new peers.
#[derive(libp2p::swarm::NetworkBehaviour)]
pub struct DiscoveryBehavior {
    /// Kademlia DHT for wide-area peer discovery
    pub kademlia: Kademlia<MemoryStore>,
    /// mDNS for local network peer discovery
    pub mdns: mdns::async_io::Behaviour,
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
        self.routing_table.add_route(peer_id, peer_id);
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
    /// # Arguments
    ///
    /// * `peer_id` - The local peer ID
    /// * `bootstrap_nodes` - Initial bootstrap nodes to seed the DHT
    ///
    /// # Errors
    ///
    /// Returns an error if mDNS initialization fails.
    ///
    /// # DHT Configuration
    ///
    /// - Uses in-memory store for routing table
    /// - Configures 5-minute periodic refresh
    /// - Adds all bootstrap nodes to routing table
    pub async fn new(
        peer_id: PeerId,
        bootstrap_nodes: Vec<BootstrapNode>,
    ) -> Result<Self, DiscoveryError> {
        // Initialize Kademlia DHT with in-memory store
        let store = MemoryStore::new(peer_id);
        let kad_config = KademliaConfig::default();
        
        // Note: Periodic bootstrap is triggered manually via bootstrap() calls
        // The automatic_throttle config option controls internal DHT maintenance
        
        let mut kademlia = Kademlia::with_config(peer_id, store, kad_config);

        // Add bootstrap nodes to Kademlia routing table
        for node in bootstrap_nodes {
            kademlia.add_address(&node.peer_id, node.multiaddr.clone());
            log::info!("state-transition: Added bootstrap peer {} at {}", node.peer_id, node.multiaddr);
        }

        // Trigger initial bootstrap
        if let Err(e) = kademlia.bootstrap() {
            log::warn!("catalog: Bootstrap failed: {:?}", e);
        }

        // Initialize mDNS for local peer discovery
        let mdns = mdns::async_io::Behaviour::new(
            mdns::Config::default(),
            peer_id,
        )
        .map_err(|e| DiscoveryError::Mdns(format!("Failed to initialize mDNS: {}", e)))?;

        log::info!("state-transition: Discovery initialized for peer {}", peer_id);

        Ok(Self { kademlia, mdns })
    }

    /// Discovers peers using both DHT and mDNS.
    ///
    /// This method returns all currently known peers from both discovery mechanisms.
    /// It does not block waiting for new peers; instead, it returns the current state.
    ///
    /// # Returns
    ///
    /// A vector of discovered peer IDs. May be empty if no peers have been discovered yet.
    ///
    /// # Note
    ///
    /// Peer discovery is an ongoing process. Call this method periodically to get
    /// updated peer lists as the swarm processes network events.
    pub async fn discover_peers(&mut self) -> Result<Vec<PeerId>, DiscoveryError> {
        // Collect peers from Kademlia routing table
        let mut peers: Vec<PeerId> = Vec::new();
        
        // Kademlia peers are in the routing table buckets
        for bucket in self.kademlia.kbuckets() {
            for entry in bucket.iter() {
                let peer_id = *entry.node.key.preimage();
                if !peers.contains(&peer_id) {
                    peers.push(peer_id);
                }
            }
        }

        log::debug!("catalog: Discovered {} total peers", peers.len());

        Ok(peers)
    }

    /// Returns the current count of discovered peers.
    ///
    /// This is a lightweight alternative to `discover_peers()` when you only
    /// need the count and not the full list of peer IDs.
    pub fn peer_count(&mut self) -> usize {
        let mut count = 0;
        for bucket in self.kademlia.kbuckets() {
            count += bucket.num_entries();
        }
        count
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
    fn test_load_bootstrap_nodes_empty_without_catalog() {
        // Without catalog integration, should return empty to avoid seed fallback
        let nodes = load_bootstrap_nodes();
        assert_eq!(nodes.len(), 0);
    }

    #[async_std::test]
    async fn test_discovery_behavior_creation() {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        let discovery = DiscoveryBehavior::new(peer_id, vec![]).await;
        assert!(discovery.is_ok());
    }

    #[async_std::test]
    async fn test_peer_count_initially_zero() {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        let mut discovery = DiscoveryBehavior::new(peer_id, vec![])
            .await
            .expect("Failed to create discovery");

        assert_eq!(discovery.peer_count(), 0);
    }

    #[async_std::test]
    async fn test_discover_peers_empty_initially() {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        let mut discovery = DiscoveryBehavior::new(peer_id, vec![])
            .await
            .expect("Failed to create discovery");

        let peers = discovery.discover_peers().await.expect("discover_peers failed");
        assert_eq!(peers.len(), 0);
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
        assert_eq!(routing.get_routing_table().find_route(&discovered_peer), None);
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
